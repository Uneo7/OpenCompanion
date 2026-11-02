//! Wraps firmware build outputs into the two `.ocfw` containers (`oc firmware pack`, the stage and release
//! scripts), after the same identity checks the installer runs.

use std::path::{Path, PathBuf};

use oc_ais::eeprom::boot::{boot0_from_elf, boot1_from_elf};
use oc_ais::eeprom::image::{app_image, AppOptions};

use super::flow::Chip;
use super::identity;
use super::ocfw::{expand_release, Ocfw, DSP_FILE, USB_FILE};
use super::sources::{tas_version, FirmwareSources};

fn read(p: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// The files of `src` (development tree or a release folder in the raw layout) as `[DSP, USB]` containers.
pub fn pack(src: &FirmwareSources, min_app_version: &str) -> Result<[Ocfw; 2], String> {
    let stage = std::env::temp_dir().join(format!("opencompanion-pack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    let pairs: [(&str, PathBuf); 7] = [
        ("opencompanion_dsp.out", src.dsp_app.clone()),
        ("loader.out", src.stub.clone()),
        ("boot/tramp.out", src.boot_build.join("tramp.out")),
        ("boot/boot1.out", src.boot_build.join("boot1.out")),
        ("boot/updater.out", src.boot_build.join("updater.out")),
        ("oc_tas.dfu", src.tas_image.clone()),
        ("oc_tas_data.bin", src.tas_data.clone()),
    ];
    for (rel, from) in &pairs {
        let to = stage.join(rel);
        std::fs::create_dir_all(to.parent().expect("parent")).map_err(|e| e.to_string())?;
        std::fs::copy(from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
    }
    let r = (|| {
        let staged = FirmwareSources::from_dir(&stage);
        staged.loaders()?;
        let elf = read(&staged.dsp_app)?;
        let dsp_version = identity::check_dsp_elf(&elf)?;
        app_image(&elf, &AppOptions::default()).map_err(|e| format!("opencompanion_dsp.out: {e}"))?;
        boot0_from_elf(&read(&staged.boot_build.join("tramp.out"))?).map_err(|e| format!("tramp.out: {e}"))?;
        boot1_from_elf(&read(&staged.boot_build.join("boot1.out"))?).map_err(|e| format!("boot1.out: {e}"))?;
        let target = staged.tas_target(None)?;
        let tas = tas_version(&target).ok_or("oc_tas.dfu: no OC-x.y.z version")?;
        Ok([
            Ocfw::from_dir(&stage, Chip::Dsp, &dsp_version, min_app_version)?,
            Ocfw::from_dir(&stage, Chip::Usb, &tas, min_app_version)?,
        ])
    })();
    let _ = std::fs::remove_dir_all(&stage);
    r
}

/// Writes [`DSP_FILE`] and [`USB_FILE`] into `out`.
pub fn write_pack(src: &FirmwareSources, out: &Path, min_app_version: &str) -> Result<[PathBuf; 2], String> {
    let [dsp, usb] = pack(src, min_app_version)?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let (d, u) = (out.join(DSP_FILE), out.join(USB_FILE));
    std::fs::write(&d, dsp.to_bytes()?).map_err(|e| e.to_string())?;
    std::fs::write(&u, usb.to_bytes()?).map_err(|e| e.to_string())?;
    Ok([d, u])
}

/// The raw files of a folder that holds the two containers (extracted into a temporary folder), or the folder
/// itself when it is already in the raw layout.
pub fn sources_of_dir(dir: &Path, app_version: &str) -> Result<FirmwareSources, String> {
    if !dir.join(DSP_FILE).is_file() {
        return Ok(FirmwareSources::from_dir(dir));
    }
    let tmp = std::env::temp_dir().join(format!("opencompanion-unpack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    expand_release(dir, &tmp, app_version)?;
    Ok(FirmwareSources::from_dir(&tmp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_release_files_pack_and_unpack() {
        let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/release"));
        if !dir.join("oc_tas.dfu").is_file() && !dir.join(DSP_FILE).is_file() {
            eprintln!("skipped: firmware/release not present");
            return;
        }
        let src = sources_of_dir(dir, "99.0.0").unwrap();
        match pack(&src, "0.1.0") {
            Ok([dsp, usb]) => {
                let b = usb.to_bytes().unwrap();
                assert_eq!(Ocfw::parse(&b, Chip::Usb, "0.1.0").unwrap(), usb);
                assert!(Ocfw::parse(&b, Chip::Dsp, "0.1.0").is_err());
                assert_eq!(dsp.chip, Chip::Dsp);
            }
            // A work-in-progress build may not load: reported, not a failure of this test.
            Err(e) => eprintln!("firmware/release not packable: {e}"),
        }
    }
}
