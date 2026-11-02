mod sim;

use std::rc::Rc;

use oc_install::restore::{reboot_tas, restore_tas, stock_tas_target, TasRestoreReport};
use oc_install::tas::{OcPort, StockPort, TasContent, TasOptions};
use oc_install::{Error, UiStep};
use sim::fixture;
use sim::tas::TasRun;
use sim::{BoseLink, Fault, FaultKind, Known, OcSim, Shared, Speaker, TestHost};

fn known() -> Rc<Known> {
    let mut rng = fixture::Rng::new(7);
    let t = fixture::tas_target_with_data(1, 5963, 0xF0, 337);
    Rc::new(Known { dsp: fixture::bundle(1, 1), tas_bose: fixture::tas_bose(&mut rng), tas_oc: vec![t] })
}

fn oc_speaker() -> Shared {
    let sp = Speaker::new(known());
    let target = sp.borrow().known.tas_oc[0].bytes.clone();
    sp.borrow_mut().tas.ee = target;
    sp.borrow_mut().power_cycle();
    assert_eq!(sp.borrow().tas.run, TasRun::Oc(0));
    sp
}

fn run(sp: &Shared, backup: &[u8], dry_run: bool) -> Option<(oc_install::Result<TasRestoreReport>, TestHost)> {
    let mut host = TestHost::new(sp, true);
    let opts = TasOptions { dry_run };
    let tas = sp.borrow().tas.run;
    let r = match tas {
        TasRun::Bose => restore_tas(&StockPort::new(&BoseLink(sp.clone())), backup, &mut host, opts),
        TasRun::Oc(_) => restore_tas(&OcPort::new(&OcSim(sp.clone())), backup, &mut host, opts),
        _ => return None,
    };
    Some((r, host))
}

fn backup(sp: &Shared) -> Vec<u8> {
    sp.borrow().known.tas_bose.clone()
}

#[test]
fn restore_through_the_opencompanion_firmware_then_reboot() {
    let sp = oc_speaker();
    let bk = backup(&sp);
    let (r, host) = run(&sp, &bk, false).unwrap();
    let rep = r.unwrap();
    assert!(matches!(rep.before, TasContent::OtherApplication { .. }));
    assert!(rep.restored && rep.pages_written > 0);
    assert_eq!(host.steps, [UiStep::RestoreUsb, UiStep::UsbVerify]);
    assert_eq!(sp.borrow().tas.ee, bk, "all 8 KB = backup (data area erased too)");
    let mut host = TestHost::new(&sp, true);
    reboot_tas(&OcSim(sp.clone()), &mut host).unwrap();
    assert_eq!(sp.borrow().tas.run, TasRun::Bose, "the Bose TAS firmware starts");
    let w = sp.borrow().ee_writes;
    let rep = run(&sp, &bk, false).unwrap().0.unwrap();
    assert!(rep.restored && rep.pages_written == 0 && rep.before == TasContent::Target);
    assert_eq!(sp.borrow().ee_writes, w);
}

#[test]
fn dry_run_writes_nothing_and_bad_backups_are_refused() {
    let sp = oc_speaker();
    let bk = backup(&sp);
    let rep = run(&sp, &bk, true).unwrap().0.unwrap();
    assert!(!rep.restored);
    assert_eq!(sp.borrow().ee_writes, 0);
    let oc_image = sp.borrow().tas.ee.clone();
    assert!(matches!(stock_tas_target(&oc_image), Err(Error::Refused(_))), "a post-install dump is not stock");
    assert!(stock_tas_target(&bk[..4096]).is_err());
    assert!(matches!(run(&sp, &oc_image, false).unwrap().0, Err(Error::Refused(_))));
    assert_eq!(sp.borrow().ee_writes, 0);
}

#[test]
fn resumes_through_the_stock_path_or_needs_dfu_after_a_power_cut() {
    let bk = backup(&oc_speaker());
    let total = {
        let sp = oc_speaker();
        let start = sp.borrow().ops;
        run(&sp, &bk, false).unwrap().0.unwrap();
        let end = sp.borrow().ops;
        end - start
    };
    let (mut dfu, mut resumed) = (0, 0);
    for kind in [FaultKind::HostCrash, FaultKind::PowerBefore, FaultKind::PowerMid] {
        for at in 0..total {
            let sp = oc_speaker();
            let start = sp.borrow().ops;
            sp.borrow_mut().fault = Some(Fault { at: start + at, kind });
            let first = run(&sp, &bk, false).unwrap().0;
            sp.borrow_mut().reconnect();
            if first.is_ok() {
                continue;
            }
            match sp.borrow().tas.run {
                // Power cut between the DataType 3 write and the final DataType 1: documented ROM DFU recovery.
                TasRun::Dfu => {
                    assert_ne!(kind, FaultKind::HostCrash);
                    dfu += 1;
                    continue;
                }
                TasRun::Dead => panic!("{kind:?} at {at}: the TAS boots unknown code"),
                _ => {}
            }
            let (second, host) = run(&sp, &bk, false).expect("vendor HID present");
            if let Err(e) = second {
                panic!("resume after {kind:?} at {at}: {e}\n{}", host.log.join("\n"));
            }
            assert_eq!(sp.borrow().tas.ee, bk);
            resumed += 1;
        }
    }
    assert!(dfu > 0 && resumed > 0);
}
