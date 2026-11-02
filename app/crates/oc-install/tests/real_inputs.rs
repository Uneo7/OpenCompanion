//! Skipped when the real build outputs of this repository are missing.

use std::path::{Path, PathBuf};

use oc_ais::eeprom::boot::{boot0_from_elf, boot1_from_elf, updater_from_elf};
use oc_ais::eeprom::image::{app_image, AppOptions};
use oc_install::dsp::DspBundle;
use oc_install::tas::TasTarget;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").join(rel)
}

fn read(rel: &str) -> Option<Vec<u8>> {
    std::fs::read(repo(rel)).ok()
}

#[test]
fn real_dsp_bundle_is_valid() {
    let files = (
        read("../u7001_dsp.bin"),
        read("firmware/dsp/boot/build/tramp.out"),
        read("firmware/dsp/boot/build/boot1.out"),
        read("firmware/dsp/boot/build/updater.out"),
        read("firmware/dsp/build/opencompanion_dsp.out"),
    );
    let (Some(stock), Some(tramp), Some(boot1), Some(updater), Some(app)) = files else {
        eprintln!("skipped: build outputs not present");
        return;
    };
    updater_from_elf(&updater).unwrap();
    let opts = AppOptions::default(); // the firmware must declare oc_eeprom_layout_v2
    let b = DspBundle {
        stock,
        stub: oc_ramload::read_image(&repo("firmware/dsp/loader/build/loader.out")).unwrap(),
        updater: oc_ramload::read_image(&repo("firmware/dsp/boot/build/updater.out")).unwrap(),
        boot0: boot0_from_elf(&tramp).unwrap(),
        boot1: boot1_from_elf(&boot1).unwrap(),
        app: app_image(&app, &opts).unwrap(),
    };
    b.validate().unwrap();
}

#[test]
fn real_tas_image_is_valid() {
    let Some(img) = read("firmware/usb/build/oc_tas/oc_tas.dfu") else {
        eprintln!("skipped: oc_tas.dfu not built");
        return;
    };
    let t = TasTarget::new(oc_tas_image::Image::parse(&img).unwrap()).unwrap();
    assert_eq!(t.bytes.len(), 8192);
}

#[test]
fn real_tas_data_area_is_valid() {
    let (Some(img), Some(data)) =
        (read("firmware/usb/build/oc_tas/oc_tas.dfu"), read("firmware/usb/build/oc_tas/oc_tas_data.bin"))
    else {
        eprintln!("skipped: oc_tas.dfu / oc_tas_data.bin not built");
        return;
    };
    let area = oc_tas_image::data_area::DataArea::parse(&data).unwrap();
    assert_eq!(area.to_bytes(), data, "the build script and the host agree on the format");
    let t = TasTarget::with_data(oc_tas_image::Image::parse(&img).unwrap(), area).unwrap();
    assert_eq!(t.bytes[0x1800..0x1800 + data.len()], data[..]);
}

#[test]
fn stock_loader_digests_match_a_real_dump() {
    let Some(stock) = read("../u7001_dsp.bin") else {
        eprintln!("skipped: no stock dump");
        return;
    };
    assert_eq!(oc_install::dsp::loader_digests(&stock).unwrap(), oc_install::dsp::STOCK_LOADER_DIGESTS);
    // The real stock image can be restored in the R1..R4 order (AIS stream complete below 0xDB00).
    assert_eq!(oc_install::backup::Content::of_dsp(&stock), oc_install::backup::Content::Stock);
}
