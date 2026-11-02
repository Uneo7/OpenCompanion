//! Status must never write anything.

mod sim;

use std::rc::Rc;

use oc_install::dsp::{install, DspOptions, DspRuntime};
use oc_install::status::{dsp, tas_bose, tas_oc, tas_without_hid, TasStatus};
use oc_install::tas::TasContent;
use oc_ais::eeprom::layout::{EE_PAGE, HDR};
use oc_protocol::oc_hid::OcRelay;
use sim::dsp::DspRun;
use sim::fixture;
use sim::{BoseLink, Known, OcSim, Speaker, TestHost};

fn speaker() -> sim::Shared {
    let mut rng = fixture::Rng::new(7);
    let b = fixture::bundle(1, 1);
    Speaker::new(Rc::new(Known { dsp: b, tas_bose: fixture::tas_bose(&mut rng), tas_oc: vec![fixture::tas_target(1, 5000, 0xF0)] }))
}

#[test]
fn stock_speaker() {
    let sp = speaker();
    let link = BoseLink(sp.clone());
    assert_eq!(tas_bose(&link), TasStatus::Bose { eeprom: TasContent::Bose });
    let d = dsp(&link);
    assert!(matches!(d.runtime, DspRuntime::Stock { .. }));
    assert!(d.to_string().contains("stock"));
    assert_eq!(sp.borrow().ee_writes, 0);
}

#[test]
fn migrated_speaker_with_opencompanion_tas() {
    let sp = speaker();
    let b = sp.borrow().known.dsp.clone();
    install(&BoseLink(sp.clone()), &b, &mut TestHost::new(&sp, true), DspOptions::default()).unwrap();
    let target = sp.borrow().known.tas_oc[0].bytes.clone();
    sp.borrow_mut().tas.ee = target;
    sp.borrow_mut().power_cycle();
    let writes = sp.borrow().ee_writes;

    let oc = OcSim(sp.clone());
    match tas_oc(&oc) {
        TasStatus::OpenCompanion { info, eeprom } => {
            assert_eq!(info.version.1, 0xF0);
            assert!(matches!(eeprom, TasContent::OtherApplication { firmware_version: 0xF0, .. }));
        }
        other => panic!("{other:?}"),
    }
    let relay = OcRelay::new(&oc);
    assert_eq!(dsp(&relay).runtime, DspRuntime::OcFirmware { version: "OC0.0.1".into() });

    // Image header gone: rescue mode, reason 3.
    sp.borrow_mut().dsp.ee[HDR..HDR + EE_PAGE].fill(0);
    sp.borrow_mut().power_cycle();
    assert_eq!(dsp(&relay).runtime, DspRuntime::Rescue { reason: 3, boot1: 1 });
    assert_eq!(sp.borrow().ee_writes, writes, "status never writes");
}

#[test]
fn updater_running_reports_the_eeprom() {
    let sp = speaker();
    sp.borrow_mut().dsp.run = DspRun::Updater;
    let d = dsp(&BoseLink(sp.clone()));
    assert!(matches!(d.runtime, DspRuntime::Updater { .. }));
    assert!(d.eeprom.unwrap().contains("stock boot0"));
}

#[test]
fn no_hid_means_rom_dfu_or_absent() {
    assert_eq!(tas_without_hid(&[(0x1234, 0x5678), (0x0451, 0x1021)]), TasStatus::RomDfu { vid: 0x0451, pid: 0x1021 });
    assert_eq!(tas_without_hid(&[]), TasStatus::NotFound);
}

#[test]
fn silent_dsp() {
    let sp = speaker();
    sp.borrow_mut().dsp.run = DspRun::Dead;
    assert!(matches!(dsp(&BoseLink(sp.clone())).runtime, DspRuntime::Silent(_)));
}
