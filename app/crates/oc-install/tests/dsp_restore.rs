mod sim;

use std::rc::Rc;

use oc_ais::eeprom::layout::{BOOT1, EE_PAGE, HDR, PROFILES};
use oc_install::dsp::{install, DspBundle, DspOptions, DspRuntime};
use oc_install::restore::{restore_dsp, DspRestoreInputs, DspRestoreOptions, DspRestoreReport, RestoreStep};
use oc_install::{Error, UiStep};
use oc_protocol::oc_hid::OcRelay;
use sim::dsp::DspRun;
use sim::fixture;
use sim::tas::TasRun;
use sim::{BoseLink, Fault, FaultKind, Known, OcSim, Shared, Speaker, TestHost, ALL_FAULTS};

fn known(b: &DspBundle) -> Rc<Known> {
    let mut rng = fixture::Rng::new(7);
    Rc::new(Known { dsp: b.clone(), tas_bose: fixture::tas_bose(&mut rng), tas_oc: vec![fixture::tas_target(1, 5000, 0xF0)] })
}

fn inputs(b: &DspBundle) -> DspRestoreInputs {
    DspRestoreInputs { image: b.stock.clone(), stub: b.stub.clone(), updater: b.updater.clone() }
}

fn with_link<R>(sp: &Shared, f: impl FnOnce(&dyn oc_protocol::Link) -> R) -> R {
    let tas = sp.borrow().tas.run;
    match tas {
        TasRun::Oc(_) => {
            let oc = OcSim(sp.clone());
            f(&OcRelay::new(&oc))
        }
        _ => f(&BoseLink(sp.clone())),
    }
}

fn run(sp: &Shared, img: &DspRestoreInputs, answer: bool, dry_run: bool) -> (oc_install::Result<DspRestoreReport>, TestHost) {
    let mut host = TestHost::new(sp, answer);
    let r = with_link(sp, |l| restore_dsp(l, img, &mut host, DspRestoreOptions { dry_run }));
    (r, host)
}

fn migrated(b: &DspBundle) -> Shared {
    let sp = Speaker::new(known(b));
    let mut host = TestHost::new(&sp, true);
    install(&BoseLink(sp.clone()), b, &mut host, DspOptions::default()).unwrap();
    sp.borrow_mut().dsp.ee[PROFILES.start..PROFILES.start + EE_PAGE].fill(0x42);
    sp
}

fn assert_stock(sp: &Shared, b: &DspBundle) {
    let s = sp.borrow();
    assert_eq!(s.dsp.ee, b.stock, "all 64 KB equal the backup");
    assert_eq!(s.dsp.run, DspRun::Stock, "the Bose firmware runs");
    assert!(!s.awake, "keep-awake released");
}

#[test]
fn restore_from_opencompanion() {
    let b = fixture::bundle(1, 1);
    let sp = migrated(&b);
    let img = inputs(&b);
    let (r, host) = run(&sp, &img, true, false);
    let rep = r.unwrap();
    assert!(matches!(rep.before, DspRuntime::OcFirmware { .. }));
    assert!(rep.restored && matches!(rep.after, DspRuntime::Stock { .. }));
    assert_eq!(rep.crc32, Some(crc32fast::hash(&b.stock)));
    let kinds: Vec<_> = rep.plan.steps.iter().map(std::mem::discriminant).collect();
    let want = [
        RestoreStep::InvalidateHeader,
        RestoreStep::Body { pages: vec![] },
        RestoreStep::Tail { pages: vec![] },
        RestoreStep::StockHeader,
        RestoreStep::Boot0,
        RestoreStep::Boot1Area { pages: vec![] },
    ];
    assert_eq!(kinds, want.iter().map(std::mem::discriminant).collect::<Vec<_>>());
    assert_eq!(host.plans.len(), 1, "exactly one confirmation");
    assert!(host.plans[0].writes.iter().any(|w| w.contains("CRITICAL WINDOW")));
    assert_eq!(
        host.steps,
        [UiStep::RestoreCheck, UiStep::RestoreDsp, UiStep::DspCritical, UiStep::DspVerify],
        "user-facing steps in order, never going back"
    );
    let s = sp.borrow();
    assert_eq!(s.boot0_ops.len(), 2, "migration + restore: boot0 written once each");
    assert!(s.boot0_ops[1].1, "keep-awake during the restore window");
    drop(s);
    assert_stock(&sp, &b);
}

#[test]
fn restore_from_rescue_through_the_opencompanion_tas() {
    let b = fixture::bundle(1, 1);
    let sp = migrated(&b);
    let target = sp.borrow().known.tas_oc[0].bytes.clone();
    {
        let mut s = sp.borrow_mut();
        s.tas.ee = target;
        s.dsp.ee[HDR..HDR + EE_PAGE].fill(0); // image invalidated: rescue at power-up
        s.power_cycle();
        assert!(matches!(s.tas.run, TasRun::Oc(0)) && s.dsp.run == DspRun::Rescue(3));
    }
    run(&sp, &inputs(&b), true, false).0.unwrap();
    assert_stock(&sp, &b);
}

#[test]
fn dry_run_and_cancel_write_nothing() {
    let b = fixture::bundle(1, 1);
    let sp = migrated(&b);
    let (w, ee) = (sp.borrow().ee_writes, sp.borrow().dsp.ee.clone());
    let (r, host) = run(&sp, &inputs(&b), true, true);
    let rep = r.unwrap();
    assert!(!rep.restored && rep.plan.has_boot0() && host.plans.is_empty());
    assert!(matches!(rep.after, DspRuntime::OcFirmware { .. }), "back to our firmware");
    let (r, _) = run(&sp, &inputs(&b), false, false);
    assert!(matches!(r, Err(Error::Cancelled)));
    assert_eq!(sp.borrow().ee_writes, w);
    assert_eq!(sp.borrow().dsp.ee, ee);
    assert_eq!(sp.borrow().dsp.run, DspRun::Oc);
}

#[test]
fn refuses_backups_that_cannot_be_restored() {
    let b = fixture::bundle(1, 1);
    let sp = migrated(&b);
    // A backup of another speaker (its never-written last page differs).
    let mut other = inputs(&b);
    other.image[0xFFF0] ^= 1;
    assert!(matches!(run(&sp, &other, true, false).0, Err(Error::Refused(_))));
    let mut after = inputs(&b);
    after.image = sp.borrow().dsp.ee.clone();
    assert!(matches!(run(&sp, &after, true, false).0, Err(Error::Refused(_))));
    let mut short = inputs(&b);
    short.image.truncate(0x8000);
    assert!(matches!(run(&sp, &short, true, false).0, Err(Error::Refused(_))));
    assert_eq!(sp.borrow().boot0_ops.len(), 1, "boot0 never touched");
    assert_eq!(sp.borrow().dsp.run, DspRun::Oc);
}

#[test]
fn stock_speaker_is_verified_without_writing() {
    let b = fixture::bundle(1, 1);
    let sp = Speaker::new(known(&b));
    let (r, host) = run(&sp, &inputs(&b), true, false);
    let rep = r.unwrap();
    assert!(rep.plan.is_empty() && rep.restored && host.plans.is_empty());
    assert_eq!(sp.borrow().ee_writes, 0);
    assert_stock(&sp, &b);
}

#[test]
fn rerun_after_restore_is_a_no_op() {
    let b = fixture::bundle(1, 1);
    let sp = migrated(&b);
    run(&sp, &inputs(&b), true, false).0.unwrap();
    let w = sp.borrow().ee_writes;
    let rep = run(&sp, &inputs(&b), true, false).0.unwrap();
    assert!(rep.plan.is_empty() && rep.restored);
    assert_eq!(sp.borrow().ee_writes, w);
}

#[test]
fn cancelling_a_migration_after_m2_only_clears_boot1() {
    // M2 done (boot1 written above the stock image), M3 never happened: only R4 is needed.
    let b = fixture::bundle(1, 1);
    let sp = Speaker::new(known(&b));
    sp.borrow_mut().dsp.ee[BOOT1].copy_from_slice(&b.boot1);
    let rep = run(&sp, &inputs(&b), true, false).0.unwrap();
    assert_eq!(rep.plan.steps.len(), 1);
    assert!(matches!(rep.plan.steps[0], RestoreStep::Boot1Area { .. }));
    assert_eq!(sp.borrow().boot0_ops.len(), 0, "no critical window");
    assert_stock(&sp, &b);
}

/// Returns true if the speaker was bricked.
fn interrupt_and_resume(b: &DspBundle, at: usize, kind: FaultKind) -> bool {
    let sp = migrated(b);
    let img = inputs(b);
    let start = sp.borrow().ops;
    let boot0_before = sp.borrow().boot0_ops.len();
    sp.borrow_mut().fault = Some(Fault { at: start + at, kind });
    let (first, _) = run(&sp, &img, true, false);
    let fired = sp.borrow().fault.is_none();
    sp.borrow_mut().reconnect();
    if !fired {
        first.unwrap();
        assert_stock(&sp, b);
        return false;
    }
    assert!(first.is_err(), "a fault at {at} ({kind:?}) must surface as an error");
    {
        let s = sp.borrow();
        assert!(s.boot0_ops[boot0_before..].iter().all(|(_, awake)| *awake), "boot0 written without keep-awake");
        if s.dsp.run == DspRun::Dead {
            let at_boot0 = s.boot0_ops.last().is_some_and(|(op, _)| *op == start + at);
            assert!(kind == FaultKind::PowerMid && at_boot0, "bricked by {kind:?} at op {at}, outside the critical window");
            return true;
        }
    }
    let (second, host) = run(&sp, &img, true, false);
    if let Err(e) = &second {
        panic!("resume after {kind:?} at op {at} failed: {e}\n{}", host.log.join("\n"));
    }
    assert_stock(&sp, b);
    false
}

#[test]
fn restore_resumes_after_any_interruption() {
    let b = fixture::bundle(1, 1);
    let total = {
        let sp = migrated(&b);
        let start = sp.borrow().ops;
        run(&sp, &inputs(&b), true, false).0.unwrap();
        let end = sp.borrow().ops;
        end - start
    };
    assert!(total > 1000, "{total}");
    // One thread per fault kind (the simulator is single-threaded, each run builds its own speaker).
    let bricks: usize = std::thread::scope(|s| {
        let handles: Vec<_> = ALL_FAULTS
            .iter()
            .map(|&kind| {
                let b = b.clone();
                s.spawn(move || (0..total).filter(|&at| interrupt_and_resume(&b, at, kind)).count())
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });
    // Exactly one transaction (the stock EE_WRITE_BOOT0) can brick, and only with a power cut in the middle.
    assert_eq!(bricks, 1, "{total} transactions swept");
}

/// TAS 0.4.9 with the stock DSP: after the reboot the relay returns a stale frame to `SVM`. U7001 was verified
/// byte for byte: the restore succeeds with a warning (the TAS step comes next).
#[test]
fn a_garbled_version_reply_after_a_verified_restore_is_only_a_warning() {
    let b = fixture::bundle(1, 1);
    let sp = migrated(&b);
    sp.borrow_mut().dsp.garble_after_boot = true;
    let (r, host) = run(&sp, &inputs(&b), true, false);
    let rep = r.expect("restore goes on");
    assert!(rep.restored && matches!(rep.after, DspRuntime::Silent(_)));
    assert!(host.log.iter().any(|l| l.starts_with("WARNING") && l.contains("verified byte for byte")));
    assert_eq!(sp.borrow().dsp.ee, b.stock);
}

/// The same after an install: every page was read back, a garbled version reply does not fail it.
#[test]
fn a_garbled_version_reply_after_a_verified_install_is_only_a_warning() {
    let b = fixture::bundle(1, 1);
    let sp = Speaker::new(known(&b));
    sp.borrow_mut().dsp.garble_after_boot = true;
    let mut host = TestHost::new(&sp, true);
    let rep = install(&BoseLink(sp.clone()), &b, &mut host, DspOptions::default()).expect("install goes on");
    assert!(rep.installed);
    assert!(host.log.iter().any(|l| l.starts_with("WARNING")));
}
