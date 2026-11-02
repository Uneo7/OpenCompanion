mod sim;

use std::rc::Rc;

use oc_install::tas::{
    install, write_data_area, DataOnly, OcPort, StockPort, TasContent, TasOptions, TasPort, TasReport, TasTarget, DATA_AREA,
};
use oc_install::Error;
use sim::fixture;
use sim::tas::TasRun;
use sim::{BoseLink, Fault, FaultKind, Known, OcSim, Shared, Speaker, TestHost, ALL_FAULTS};

fn known(targets: Vec<TasTarget>) -> Rc<Known> {
    let mut rng = fixture::Rng::new(7);
    Rc::new(Known { dsp: fixture::bundle(1, 1), tas_bose: fixture::tas_bose(&mut rng), tas_oc: targets })
}

/// `None` = no vendor HID (ROM DFU).
fn run(sp: &Shared, t: &TasTarget, answer: bool, dry_run: bool) -> Option<(oc_install::Result<TasReport>, TestHost)> {
    let mut host = TestHost::new(sp, answer);
    let opts = TasOptions { dry_run };
    let tas = sp.borrow().tas.run;
    let r = match tas {
        TasRun::Bose => install(&StockPort::new(&BoseLink(sp.clone())), t, &mut host, opts),
        TasRun::Oc(_) => install(&OcPort::new(&OcSim(sp.clone())), t, &mut host, opts),
        _ => return None,
    };
    Some((r, host))
}

fn assert_installed(sp: &Shared, t: &TasTarget, index: usize) {
    let mut s = sp.borrow_mut();
    assert_eq!(s.tas.ee, t.bytes, "whole EEPROM = image");
    assert!(!s.awake);
    s.power_cycle();
    assert_eq!(s.tas.run, TasRun::Oc(index), "the new firmware boots");
}

#[test]
fn first_install_through_the_stock_firmware() {
    let t = fixture::tas_target(1, 5963, 0xF0);
    let sp = Speaker::new(known(vec![t.clone()]));
    let (r, host) = run(&sp, &t, true, false).unwrap();
    let rep = r.unwrap();
    assert_eq!(rep.before, TasContent::Bose);
    assert!(rep.installed);
    assert_eq!(host.plans.len(), 1);
    assert!(host.plans[0].writes[0].contains("DataType := 3"));
    // DataType 3 first, final header last with DataType alone at the very end.
    let ops = sp.borrow().tas_header_ops.clone();
    assert!(ops.len() >= 4);
    assert_installed(&sp, &t, 0);
}

#[test]
fn shorter_image_erases_the_stock_tail() {
    let t = fixture::tas_target(1, 1000, 0xF0);
    let sp = Speaker::new(known(vec![t.clone()]));
    run(&sp, &t, true, false).unwrap().0.unwrap();
    assert!(sp.borrow().tas.ee[18 + 1000..].iter().all(|&b| b == 0xFF));
    assert_installed(&sp, &t, 0);
}

#[test]
fn dry_run_and_cancel_write_nothing() {
    let t = fixture::tas_target(1, 5963, 0xF0);
    let sp = Speaker::new(known(vec![t.clone()]));
    let (r, host) = run(&sp, &t, true, true).unwrap();
    assert!(!r.unwrap().installed);
    assert!(host.plans.is_empty());
    let (r, _) = run(&sp, &t, false, false).unwrap();
    assert!(matches!(r, Err(Error::Cancelled)));
    assert_eq!(sp.borrow().ee_writes, 0);
}

#[test]
fn update_through_the_opencompanion_firmware() {
    let t1 = fixture::tas_target(1, 5963, 0xF0);
    let t2 = fixture::tas_target(2, 5500, 0xF1);
    let sp = Speaker::new(known(vec![t1.clone(), t2.clone()]));
    run(&sp, &t1, true, false).unwrap().0.unwrap();
    assert_installed(&sp, &t1, 0);
    let (r, _) = run(&sp, &t2, true, false).unwrap();
    assert!(matches!(r.unwrap().before, TasContent::OtherApplication { firmware_version: 0xF0, .. }));
    assert_installed(&sp, &t2, 1);
    let w = sp.borrow().ee_writes;
    assert_eq!(run(&sp, &t2, true, false).unwrap().0.unwrap().pages_written, 0);
    assert_eq!(sp.borrow().ee_writes, w);
}

#[test]
fn opencompanion_port_refuses_bytes_it_cannot_write() {
    let t1 = fixture::tas_target(1, 5963, 0xF0);
    let sp = Speaker::new(known(vec![t1.clone()]));
    run(&sp, &t1, true, false).unwrap().0.unwrap();
    sp.borrow_mut().power_cycle();
    sp.borrow_mut().tas.ee[0x17C0] = 0x00; // between the code area and the data area: the firmware refuses it
    let t2 = fixture::tas_target(2, 5500, 0xF1);
    let (r, _) = run(&sp, &t2, true, false).unwrap();
    assert!(matches!(r, Err(Error::Refused(_))));
}

#[test]
fn ports_split_reads_and_writes_on_32_byte_pages() {
    // The simulator panics on any write crossing a page; reads are split the same way.
    let t = fixture::tas_target(3, 6016, 0xF0);
    let sp = Speaker::new(known(vec![t.clone()]));
    let link = BoseLink(sp.clone());
    let port = StockPort::new(&link);
    assert_eq!(port.read(0x1F, 40).unwrap(), sp.borrow().tas.ee[0x1F..0x1F + 40].to_vec());
    assert!(port.write(0x10, &[0; 4]).is_err(), "header bytes are not code");
    assert!(port.write(0x3E, &[0; 4]).is_err(), "page crossing refused");
}

/// Interrupt at `at`; resume when the TAS still has a vendor HID. Returns true if the TAS ended in ROM DFU.
fn interrupt_and_resume(setup: &dyn Fn() -> Shared, t: &TasTarget, index: usize, at: usize, kind: FaultKind) -> bool {
    let sp = setup();
    let start = sp.borrow().ops;
    sp.borrow_mut().fault = Some(Fault { at: start + at, kind });
    let first = run(&sp, t, true, false).unwrap().0;
    let fired = sp.borrow().fault.is_none();
    sp.borrow_mut().reconnect();
    if !fired {
        first.unwrap();
        assert_installed(&sp, t, index);
        return false;
    }
    assert!(first.is_err());
    let dfu = sp.borrow().tas.run == TasRun::Dfu;
    if dfu {
        // Only a power cut after the header left DataType 1 can do this: the documented DFU recovery.
        let s = sp.borrow();
        assert!(kind != FaultKind::HostCrash, "a host crash never changes what the TAS boots");
        assert!(s.tas_header_ops.first().is_some_and(|&op| op <= start + at), "DFU before any header write ({kind:?} at {at})");
        return true;
    }
    assert_ne!(sp.borrow().tas.run, TasRun::Dead, "{kind:?} at {at}: TAS runs unknown code");
    let (second, host) = run(&sp, t, true, false).expect("vendor HID present");
    if let Err(e) = &second {
        panic!("resume after {kind:?} at op {at} failed: {e}\n{}", host.log.join("\n"));
    }
    assert_installed(&sp, t, index);
    false
}

fn sweep(setup: &dyn Fn() -> Shared, t: &TasTarget, index: usize, kinds: &[FaultKind]) -> (usize, usize) {
    let sp = setup();
    let start = sp.borrow().ops;
    run(&sp, t, true, false).unwrap().0.unwrap();
    let total = sp.borrow().ops - start;
    let mut dfu = 0;
    for &kind in kinds {
        for at in 0..total {
            dfu += interrupt_and_resume(setup, t, index, at, kind) as usize;
        }
    }
    (total, dfu)
}

#[test]
fn stock_install_resumes_after_any_interruption() {
    let t = fixture::tas_target(1, 5963, 0xF0);
    let k = known(vec![t.clone()]);
    let (total, dfu) = sweep(&|| Speaker::new(k.clone()), &t, 0, &ALL_FAULTS);
    assert!(total > 500);
    // Host crashes always resume; power cuts between the first and the last header write give ROM DFU.
    assert!(dfu > 0 && dfu < 3 * total);
}

#[test]
fn opencompanion_update_resumes_after_host_crashes() {
    let t1 = fixture::tas_target(1, 5963, 0xF0);
    let t2 = fixture::tas_target(2, 5500, 0xF1);
    let k = known(vec![t1.clone(), t2.clone()]);
    let setup = || {
        let sp = Speaker::new(k.clone());
        run(&sp, &t1, true, false).unwrap().0.unwrap();
        sp.borrow_mut().power_cycle();
        sp
    };
    let (_, dfu) = sweep(&setup, &t2, 1, &[FaultKind::HostCrash, FaultKind::PowerBefore]);
    assert!(dfu > 0, "power cuts during the session leave DataType 3 -> ROM DFU");
}

#[test]
fn data_area_alone_through_the_stock_firmware() {
    let t = fixture::tas_target(1, 5963, 0xF0);
    let sp = Speaker::new(known(vec![t]));
    let before = sp.borrow().tas.ee.clone();
    let area = fixture::data_area(9, 337);
    let link = BoseLink(sp.clone());
    let port = StockPort::new(&link);
    let mut host = TestHost::new(&sp, true);
    let rep = write_data_area(&port, &area, &mut host, false).unwrap();
    assert!(rep.written && rep.pages_written > 0);
    let ee = sp.borrow().tas.ee.clone();
    assert_eq!(ee[DATA_AREA], area.area_bytes()[..], "data area = blob + 0xFF");
    assert_eq!(ee[..DATA_AREA.start], before[..DATA_AREA.start], "header and code untouched");
    let w = sp.borrow().ee_writes;
    assert_eq!(write_data_area(&port, &area, &mut host, false).unwrap().pages_written, 0);
    assert_eq!(sp.borrow().ee_writes, w);
    sp.borrow_mut().power_cycle();
    assert_eq!(sp.borrow().tas.run, TasRun::Bose);
}

#[test]
fn data_area_guard_refuses_anything_below_0x1800() {
    let t = fixture::tas_target(1, 5963, 0xF0);
    let sp = Speaker::new(known(vec![t]));
    let link = BoseLink(sp.clone());
    let port = StockPort::new(&link);
    let guard = DataOnly(&port);
    assert!(matches!(guard.write(0x17E0, &[0; 32]), Err(Error::Refused(_))));
    assert!(matches!(guard.write(0x0000, &[0; 18]), Err(Error::Refused(_))));
    assert!(matches!(guard.write(0x1FF0, &[0; 32]), Err(Error::Refused(_))), "past the end");
    assert_eq!(sp.borrow().ee_writes, 0);
    assert!(guard.write(0x1800, &[0; 32]).is_ok());
}

#[test]
fn install_and_update_with_a_data_area() {
    let t1 = fixture::tas_target_with_data(1, 5963, 0xF0, 337);
    let t2 = fixture::tas_target_with_data(2, 6016, 0xF1, 400);
    let sp = Speaker::new(known(vec![t1.clone(), t2.clone()]));
    run(&sp, &t1, true, false).unwrap().0.unwrap();
    assert_installed(&sp, &t1, 0);
    run(&sp, &t2, true, false).unwrap().0.unwrap(); // through the OpenCompanion firmware: code + data area
    assert_installed(&sp, &t2, 1);
}

#[test]
fn update_keeps_the_audio_mode_byte() {
    // 0x17FF = the device audio mode (HID MODE command, oc_tas 0.4.0+): a user setting that EE_WRITE refuses.
    let t1 = fixture::tas_target(1, 5963, 0xF0);
    let t2 = fixture::tas_target(2, 5500, 0xF1);
    let sp = Speaker::new(known(vec![t1.clone(), t2.clone()]));
    run(&sp, &t1, true, false).unwrap().0.unwrap();
    sp.borrow_mut().power_cycle();
    sp.borrow_mut().tas.ee[0x17FF] = 2; // mode 2.1, set by the user
    let r = run(&sp, &t2, true, false).unwrap().0.unwrap();
    assert!(r.installed);
    let s = sp.borrow();
    assert_eq!(s.tas.ee[0x17FF], 2, "the user's mode survives the update");
    assert!(s.tas.ee.iter().zip(&t2.bytes).enumerate().all(|(i, (a, b))| a == b || i == 0x17FF));
}
