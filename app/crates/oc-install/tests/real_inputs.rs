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

/// The words `[addr, addr + 4)` as initialised by the COFF `.cinit` table at `cinit` (`[len][addr][data]`
/// records, 8-byte aligned), i.e. the stock RAM after `c_int00`.
fn cinit_word(ram: &[u8], cinit: u32, addr: u32) -> Option<u32> {
    let le = |o: usize| u32::from_le_bytes(ram[o..o + 4].try_into().unwrap());
    let mut p = (cinit - oc_ramload::RAM_LO) as usize;
    let mut found = None;
    loop {
        let (n, a) = (le(p) as usize, le(p + 4));
        if n == 0 || p + 8 + n > ram.len() {
            return found;
        }
        if a <= addr && addr + 4 <= a + n as u32 {
            found = Some(le(p + 8 + (addr - a) as usize));
        }
        p += 8 + ((n + 7) & !7);
    }
}

fn check_revision(stock: &[u8], rev: &oc_install::dsp::revision::StockRevision, cinit: u32, cinit_end: u32) {
    use oc_install::backup::Content;
    assert_eq!(oc_install::dsp::revision_digests(stock, rev).unwrap(), rev.digests, "{} digests", rev.dsp);
    let ram = oc_ramload::stock_ram(stock).unwrap();
    assert!(rev.loader_hi <= cinit_end && oc_ramload::LOADER_LO >= cinit, "{}: the checked area is .cinit", rev.dsp);
    assert_eq!(cinit_word(&ram, cinit, rev.flags_addr()), Some(3), "{} flags", rev.dsp);
    assert_eq!(cinit_word(&ram, cinit, rev.slot_addr()), Some(rev.process), "{} process pointer", rev.dsp);
    if let Ok(stub) = oc_ramload::read_image(&repo("firmware/dsp/loader/build/loader.out")) {
        for (a, d) in &stub.segments {
            assert!(*a >= oc_ramload::LOADER_LO && a + d.len() as u32 <= rev.loader_hi, "{}: stub segment 0x{a:08x}", rev.dsp);
        }
    }
    assert_eq!(Content::of_dsp(stock), Content::Stock, "{}: stock image below the boot1 area", rev.dsp);
}

#[test]
fn d010101_revision_matches_a_real_dump() {
    let Some(stock) = read("../u7001_dsp.bin") else {
        eprintln!("skipped: no stock dump");
        return;
    };
    check_revision(&stock, &oc_install::dsp::revision::D010101, 0x1001_DB40, 0x1001_DB40 + 29444);
}

/// The D01.01.03 dump is a user's backup: never part of the repository. `OC_STOCK_D010103` = its path.
#[test]
fn d010103_revision_matches_a_real_dump() {
    let Some(stock) = std::env::var_os("OC_STOCK_D010103").and_then(|p| std::fs::read(p).ok()) else {
        eprintln!("skipped: OC_STOCK_D010103 not set");
        return;
    };
    check_revision(&stock, &oc_install::dsp::revision::D010103, 0x1001_D460, 0x1001_D460 + 29444);
}
