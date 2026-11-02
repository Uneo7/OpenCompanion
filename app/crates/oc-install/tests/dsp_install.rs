mod sim;

use std::rc::Rc;

use oc_ais::eeprom::graph::GraphData;
use oc_ais::eeprom::header::ImageHeader;
use oc_ais::eeprom::layout::{graph_page, BOOT0, BOOT1, BODY, EE_PAGE, HDR, LAST_PAGE, PROFILES, STATE};
use oc_install::dsp::{install, DspBundle, DspOptions, DspReport, DspRuntime, Step};
use oc_install::Error;
use oc_protocol::oc_hid::OcRelay;
use sim::dsp::DspRun;
use sim::fixture;
use sim::tas::TasRun;
use sim::{BoseLink, Fault, FaultKind, Known, OcSim, Shared, Speaker, TestHost, ALL_FAULTS};

fn known(b: &DspBundle) -> Rc<Known> {
    let mut rng = fixture::Rng::new(7);
    Rc::new(Known { dsp: b.clone(), tas_bose: fixture::tas_bose(&mut rng), tas_oc: vec![fixture::tas_target(1, 5000, 0xF0)] })
}

fn run(sp: &Shared, b: &DspBundle, answer: bool, dry_run: bool) -> (oc_install::Result<DspReport>, TestHost) {
    let mut host = TestHost::new(sp, answer);
    let opts = DspOptions { dry_run };
    let tas = sp.borrow().tas.run;
    let r = match tas {
        TasRun::Oc(_) => {
            let oc = OcSim(sp.clone());
            install(&OcRelay::new(&oc), b, &mut host, opts)
        }
        _ => install(&BoseLink(sp.clone()), b, &mut host, opts),
    };
    (r, host)
}

fn assert_installed(sp: &Shared, b: &DspBundle) {
    let s = sp.borrow();
    let ee = &s.dsp.ee;
    assert_eq!(s.dsp.run, DspRun::Oc, "DSP must run the OpenCompanion firmware");
    assert_eq!(ee[BOOT0], b.boot0[..]);
    assert_eq!(ee[BOOT1], b.boot1[..]);
    let h = ImageHeader::parse(&ee[HDR..HDR + EE_PAGE]).unwrap();
    assert_eq!(ImageHeader { seq: 0, ..h }, b.header(0));
    assert_eq!(&ee[BODY.start..BODY.start + b.app.body.bytes.len()], &b.app.body.bytes[..]);
    assert_eq!(ee[LAST_PAGE], b.stock[LAST_PAGE], "the last page is never written");
    let g = b.app.graph.as_ref().expect("fixture firmware has graph tables");
    assert_eq!(GraphData::parse(ee).as_ref(), Ok(g), "graph data pages written and readable");
    for a in std::iter::once(STATE).chain((PROFILES.start..PROFILES.end).step_by(EE_PAGE)) {
        let page = &ee[a..a + EE_PAGE];
        assert!(page != &b.stock[a..a + EE_PAGE] || page.iter().all(|&x| x == 0xFF), "stale stock page at 0x{a:04x}");
    }
    assert!(!s.awake, "keep-awake released at the end");
}

fn stock_speaker(b: &DspBundle) -> Shared {
    Speaker::new(known(b))
}

fn migrated_speaker(b: &DspBundle) -> Shared {
    let sp = stock_speaker(b);
    let (r, _) = run(&sp, b, true, false);
    r.unwrap();
    sp
}

#[test]
fn migration_from_stock() {
    let b = fixture::bundle(1, 0x0000_0100);
    let sp = stock_speaker(&b);
    let (r, host) = run(&sp, &b, true, false);
    let rep = r.unwrap();
    assert!(matches!(rep.before, DspRuntime::Stock { .. }));
    assert!(rep.installed);
    let kinds: Vec<_> = rep.plan.steps.iter().map(std::mem::discriminant).collect();
    let expected = [
        Step::Boot1 { pages: vec![] },
        Step::Boot0,
        Step::Body { pages: vec![] },
        Step::Erase { pages: vec![] },
        Step::Graph { pages: vec![] },
        Step::Header { seq: 1 },
    ];
    assert_eq!(kinds, expected.iter().map(std::mem::discriminant).collect::<Vec<_>>());
    assert_eq!(host.plans.len(), 1, "exactly one confirmation");
    assert!(host.plans[0].writes.iter().any(|w| w.contains("CRITICAL WINDOW")));
    assert_installed(&sp, &b);
    let s = sp.borrow();
    assert_eq!(s.boot0_ops.len(), 1, "boot0 written exactly once");
    assert!(s.boot0_ops[0].1, "keep-awake active during the critical window");
}

#[test]
fn dry_run_writes_nothing_and_returns_to_stock() {
    let b = fixture::bundle(1, 1);
    let sp = stock_speaker(&b);
    let (r, host) = run(&sp, &b, true, true);
    let rep = r.unwrap();
    assert!(!rep.installed);
    assert!(!rep.plan.is_empty());
    assert!(host.plans.is_empty(), "no confirmation in a dry run");
    let s = sp.borrow();
    assert_eq!(s.ee_writes, 0);
    assert_eq!(s.dsp.ee, b.stock);
    assert_eq!(s.dsp.run, DspRun::Stock);
}

#[test]
fn cancel_writes_nothing() {
    let b = fixture::bundle(1, 1);
    let sp = stock_speaker(&b);
    let (r, _) = run(&sp, &b, false, false);
    assert!(matches!(r, Err(Error::Cancelled)));
    assert_eq!(sp.borrow().ee_writes, 0);
    assert_eq!(sp.borrow().dsp.run, DspRun::Stock);
}

#[test]
fn refuses_an_eeprom_that_differs_from_the_backup() {
    let b = fixture::bundle(1, 1);
    let sp = stock_speaker(&b);
    sp.borrow_mut().dsp.ee[0x4000] ^= 1; // the stock firmware still boots in the model only if intact…
    sp.borrow_mut().dsp.run = DspRun::Stock; // …so pretend it runs (different revision)
    let (r, _) = run(&sp, &b, true, false);
    assert!(matches!(r, Err(Error::Refused(_))), "{r:?}");
    assert_eq!(sp.borrow().ee_writes, 0);
}

#[test]
fn rerun_after_install_is_a_no_op() {
    let b = fixture::bundle(1, 1);
    let sp = migrated_speaker(&b);
    let writes = sp.borrow().ee_writes;
    let (r, host) = run(&sp, &b, true, false);
    let rep = r.unwrap();
    assert!(rep.plan.is_empty() && rep.installed);
    assert!(host.plans.is_empty());
    assert_eq!(sp.borrow().ee_writes, writes);
    assert_installed(&sp, &b);
}

#[test]
fn update_keeps_profiles_through_rsc() {
    let b1 = fixture::bundle(1, 1);
    let sp = migrated_speaker(&b1);
    let profile = vec![0x42u8; EE_PAGE];
    sp.borrow_mut().dsp.ee[PROFILES.start..PROFILES.start + EE_PAGE].copy_from_slice(&profile);
    let b2 = fixture::bundle(2, 2);
    let sp2 = sp.clone();
    sp2.borrow_mut().known = known(&b2);
    let (r, _) = run(&sp, &b2, true, false);
    let rep = r.unwrap();
    assert!(matches!(rep.before, DspRuntime::OcFirmware { .. }));
    assert!(!rep.plan.has_boot0(), "boot0 is never rewritten by an update");
    assert!(matches!(rep.plan.steps[0], Step::InvalidateHeader));
    assert_installed(&sp, &b2);
    assert_eq!(&sp.borrow().dsp.ee[PROFILES.start..PROFILES.start + EE_PAGE], &profile[..]);
    let h = ImageHeader::parse(&sp.borrow().dsp.ee[HDR..HDR + EE_PAGE]).unwrap();
    assert_eq!(h.seq, 2, "sequence number = previous + 1");
}

#[test]
fn graph_pages_follow_the_firmware_and_spare_the_profile_records() {
    let b1 = fixture::bundle(1, 1);
    let sp = migrated_speaker(&b1);
    // a full profile record (632 bytes) in every copy: ends before the copy's graph data page
    let rec = vec![0x5Au8; 632];
    for k in 0..8 {
        let a = graph_page(k) - 0x280;
        sp.borrow_mut().dsp.ee[a..a + rec.len()].copy_from_slice(&rec);
    }
    let b2 = fixture::bundle(3, 3);
    sp.borrow_mut().known = known(&b2);
    let (r, _) = run(&sp, &b2, true, false);
    let rep = r.unwrap();
    assert!(rep.plan.steps.iter().any(|st| matches!(st, Step::Graph { pages } if pages.len() == 8)));
    assert_installed(&sp, &b2);
    for k in 0..8 {
        let a = graph_page(k) - 0x280;
        assert_eq!(&sp.borrow().dsp.ee[a..a + rec.len()], &rec[..], "profile record of copy {k} untouched");
    }
}

#[test]
fn update_without_ram_retention_uses_upd_after_consent() {
    let b1 = fixture::bundle(1, 1);
    let sp = migrated_speaker(&b1);
    sp.borrow_mut().dsp.ram_retained = false;
    let b2 = fixture::bundle(2, 2);
    sp.borrow_mut().known = known(&b2);
    let confirms = sp.borrow().confirms;
    let (r, _) = run(&sp, &b2, true, false);
    r.unwrap();
    assert_eq!(sp.borrow().confirms - confirms, 2, "UPD consent, then the plan");
    assert_installed(&sp, &b2);
    let (r, _) = run(&sp, &fixture::bundle(3, 3), true, true);
    assert!(matches!(r, Err(Error::Refused(_))));
}

#[test]
fn update_through_the_opencompanion_tas_relay() {
    let b1 = fixture::bundle(1, 1);
    let sp = migrated_speaker(&b1);
    // The TAS now runs our firmware: smaller relay payloads (59 bytes).
    let target = sp.borrow().known.tas_oc[0].bytes.clone();
    sp.borrow_mut().tas.ee = target;
    sp.borrow_mut().power_cycle();
    assert!(matches!(sp.borrow().tas.run, TasRun::Oc(0)));
    let b2 = fixture::bundle(2, 2);
    let k = {
        let s = sp.borrow();
        Rc::new(Known { dsp: b2.clone(), tas_bose: s.known.tas_bose.clone(), tas_oc: s.known.tas_oc.clone() })
    };
    sp.borrow_mut().known = k;
    let (r, _) = run(&sp, &b2, true, false);
    r.unwrap();
    assert_installed(&sp, &b2);
}

#[test]
fn resumes_with_half_written_boot0() {
    // M3 read-back failure left page 0 new and page 1 stock while the updater still runs from RAM.
    let b = fixture::bundle(1, 1);
    let sp = stock_speaker(&b);
    {
        let mut s = sp.borrow_mut();
        s.dsp.ee[BOOT1].copy_from_slice(&b.boot1);
        s.dsp.ee[..EE_PAGE].copy_from_slice(&b.boot0[..EE_PAGE]);
        let upd = b.updater.clone();
        for (a, d) in &upd.segments {
            let o = (*a - 0x1000_0000) as usize;
            s.dsp.ram[o..o + d.len()].copy_from_slice(d);
        }
        s.dsp.run = DspRun::Updater;
    }
    let (r, _) = run(&sp, &b, true, false);
    let rep = r.unwrap();
    assert!(matches!(rep.before, DspRuntime::Updater { .. }));
    assert_eq!(rep.plan.steps[0], Step::Boot0);
    assert_installed(&sp, &b);
}

/// Returns true if the speaker was bricked.
fn interrupt_and_resume(setup: &dyn Fn() -> Shared, b: &DspBundle, at: usize, kind: FaultKind) -> bool {
    let sp = setup();
    let start = sp.borrow().ops;
    sp.borrow_mut().fault = Some(Fault { at: start + at, kind });
    let (first, _) = run(&sp, b, true, false);
    let fired = sp.borrow().fault.is_none();
    sp.borrow_mut().reconnect();
    if !fired {
        first.unwrap();
        assert_installed(&sp, b);
        return false;
    }
    assert!(first.is_err(), "a fault at {at} ({kind:?}) must surface as an error");
    {
        let s = sp.borrow();
        assert!(s.boot0_ops.iter().all(|(_, awake)| *awake), "boot0 written without keep-awake");
        if s.dsp.run == DspRun::Dead {
            let at_boot0 = s.boot0_ops.last().is_some_and(|(op, _)| *op == start + at);
            assert!(kind == FaultKind::PowerMid && at_boot0, "bricked by {kind:?} at op {at}, outside the critical window");
            return true;
        }
    }
    let (second, host) = run(&sp, b, true, false);
    if let Err(e) = &second {
        panic!("resume after {kind:?} at op {at} failed: {e}\n{}", host.log.join("\n"));
    }
    assert_installed(&sp, b);
    false
}

fn sweep(setup: &dyn Fn() -> Shared, b: &DspBundle) -> (usize, usize) {
    let sp = setup();
    let start = sp.borrow().ops;
    run(&sp, b, true, false).0.unwrap();
    let total = sp.borrow().ops - start;
    let mut bricks = 0;
    for kind in ALL_FAULTS {
        for at in 0..total {
            bricks += interrupt_and_resume(setup, b, at, kind) as usize;
        }
    }
    (total, bricks)
}

#[test]
fn migration_resumes_after_any_interruption() {
    let b = fixture::bundle(1, 1);
    let (total, bricks) = sweep(&|| stock_speaker(&b), &b);
    assert!(total > 500);
    // Exactly one transaction (EE_WRITE_BOOT0) can brick, and only with a power cut in the middle of it.
    assert_eq!(bricks, 1, "{total} transactions swept");
}

#[test]
fn update_resumes_after_any_interruption() {
    let b1 = fixture::bundle(1, 1);
    let b2 = fixture::bundle(2, 2);
    let setup = || {
        let sp = migrated_speaker(&b1);
        sp.borrow_mut().known = known(&b2);
        sp
    };
    let (_, bricks) = sweep(&setup, &b2);
    assert_eq!(bricks, 0, "an update never bricks");
}
