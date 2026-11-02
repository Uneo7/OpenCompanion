use std::path::{Path, PathBuf};

use oc_ais::eeprom::boot::{boot0_from_elf, boot1_from_elf, updater_from_elf};
use oc_ais::eeprom::image::{app_image, AppOptions};
use oc_install::dsp::DspBundle;
use oc_install::tas::TasTarget;
use oc_ramload::LoadImage;
use oc_tas_image::data_area::DataArea;
use oc_tas_image::Image;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareSources {
    pub dsp_app: PathBuf,
    pub boot_build: PathBuf,
    pub stub: PathBuf,
    /// TAS image (`oc_tas.dfu`: 18-byte header + code).
    pub tas_image: PathBuf,
    pub tas_data: PathBuf,
    /// Root of the backups (`<root>/<speaker>/<YYYY-MM-DD_HHMMSS>/`, read from the speaker; no Bose firmware is
    /// ever shipped with the app).
    pub backups: PathBuf,
}

impl FirmwareSources {
    /// A release folder: every file side by side, `boot/` for the boot chain.
    pub fn from_dir(dir: &Path) -> Self {
        Self {
            dsp_app: dir.join("opencompanion_dsp.out"),
            boot_build: dir.join("boot"),
            stub: dir.join("loader.out"),
            tas_image: dir.join("oc_tas.dfu"),
            tas_data: dir.join("oc_tas_data.bin"),
            backups: default_backups(),
        }
    }

    pub fn dev_tree(root: &Path) -> Self {
        let fw = root.join("firmware");
        Self {
            dsp_app: fw.join("dsp/build/opencompanion_dsp.out"),
            boot_build: fw.join("dsp/boot/build"),
            stub: fw.join("dsp/loader/build/loader.out"),
            tas_image: fw.join("usb/build/oc_tas/oc_tas.dfu"),
            tas_data: fw.join("usb/build/oc_tas/oc_tas_data.bin"),
            backups: default_backups(),
        }
    }

    pub fn loaders(&self) -> Result<(LoadImage, LoadImage), String> {
        let updater_elf = read(&self.boot_build.join("updater.out"))?;
        updater_from_elf(&updater_elf).map_err(|e| format!("updater.out: {e}"))?;
        let load = |p: &Path| oc_ramload::read_image(p).map_err(|e| format!("{}: {e}", p.display()));
        Ok((load(&self.stub)?, load(&self.boot_build.join("updater.out"))?))
    }

    /// DSP bundle, validated; `stock` = the U7001 image of the user's stock backup; `app` replaces the firmware
    /// ELF (install from a file).
    pub fn dsp_bundle(&self, app: Option<&Path>, stock: Vec<u8>) -> Result<DspBundle, String> {
        let app = app.unwrap_or(&self.dsp_app);
        let (stub, updater) = self.loaders()?;
        let elf = read(app)?;
        super::identity::check_dsp_elf(&elf).map_err(|e| format!("{}: {e}", app.display()))?;
        // The image header carries the build's semantic version (the build stamp stays in the ELF string).
        let fw_version = elf_version(&elf).as_deref().and_then(encode_fw_version).unwrap_or(0);
        let opts = AppOptions { seq: 0, fw_version, trial: false, allow_legacy_app: false };
        let b = DspBundle {
            stock,
            stub,
            updater,
            boot0: boot0_from_elf(&read(&self.boot_build.join("tramp.out"))?).map_err(|e| format!("tramp.out: {e}"))?,
            boot1: boot1_from_elf(&read(&self.boot_build.join("boot1.out"))?).map_err(|e| format!("boot1.out: {e}"))?,
            app: app_image(&elf, &opts).map_err(|e| format!("{}: {e}", app.display()))?,
        };
        b.validate().map_err(|e| e.to_string())?;
        Ok(b)
    }

    pub fn dsp_app_version(&self, app: Option<&Path>) -> Option<String> {
        elf_version(&read(app.unwrap_or(&self.dsp_app)).ok()?)
    }

    pub fn target_version(&self, flow: &super::flow::Flow, chip: super::flow::Chip) -> Option<String> {
        use super::flow::{Chip, Flow};
        match (flow, chip) {
            (Flow::Restore, _) => None,
            // A `.ocfw`: the version of its header (the contents are checked when the flow runs).
            (Flow::File(c, path), _) if *c == chip => super::ocfw::peek_version(path),
            (Flow::File(..), _) => None,
            (_, Chip::Dsp) => self.dsp_app_version(None),
            (_, Chip::Usb) => self.tas_target(None).ok().as_ref().and_then(tas_version),
        }
    }

    /// TAS target; `image` replaces the TAS image (install from a file: its data area is looked for next to
    /// it as `<stem>_data.bin`, else the configured one).
    pub fn tas_target(&self, image: Option<&Path>) -> Result<TasTarget, String> {
        let (img_path, data_path) = match image {
            Some(p) => {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("oc_tas");
                let beside = p.with_file_name(format!("{stem}_data.bin"));
                (p.to_path_buf(), if beside.exists() { beside } else { self.tas_data.clone() })
            }
            None => (self.tas_image.clone(), self.tas_data.clone()),
        };
        let img = Image::parse(&read(&img_path)?).map_err(|e| format!("{}: {e}", img_path.display()))?;
        let blob = read(&data_path)?;
        let data = DataArea::parse(&blob).map_err(|e| format!("{}: {e}", data_path.display()))?;
        super::identity::check_tas(&img, &data).map_err(|e| format!("{}: {e}", img_path.display()))?;
        TasTarget::with_data(img, data).map_err(|e| e.to_string())
    }
}

pub fn default_backups() -> PathBuf {
    crate::paths::backups_dir()
}

fn read(p: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(p).map_err(|e| format!("reading {}: {e}", p.display()))
}

/// Version string found in a TAS image (`OC-x.y.z`, `FW_VERSION_STRING8` of `firmware/usb/app/config.h`).
pub fn tas_version(t: &TasTarget) -> Option<String> {
    let code = &t.image.code;
    let at = code.windows(3).position(|w| w == b"OC-")?;
    let s: String = code[at + 3..].iter().take(8).take_while(|b| b.is_ascii_digit() || **b == b'.').map(|&b| b as char).collect();
    (!s.is_empty()).then_some(s)
}

/// The firmware's version string, as `SVM` reports it without the `OC` prefix: the ELF holds the literal
/// `"D" OC_VERSION "U"` of `console.c` (`DOC0.1.0+20261101183000U`; `OC_VERSION` comes from the Makefile's
/// `FW_VERSION` and `FW_BUILD`).
pub fn elf_version(elf: &[u8]) -> Option<String> {
    let mut from = 0;
    while let Some(at) = elf[from..].windows(4).position(|w| w[0] == b'D' && &w[1..3] == b"OC" && w[3].is_ascii_digit()) {
        let start = from + at + 3;
        let s: Vec<u8> = elf[start..].iter().copied().take(48).take_while(|b| b.is_ascii_graphic() && *b != b'U').collect();
        let end = start + s.len();
        if elf.get(end) == Some(&b'U') && s.first().is_some_and(u8::is_ascii_digit) {
            return String::from_utf8(s).ok();
        }
        from = start;
    }
    None
}

/// Header `fw_version` of a version string: `0x00MMmmpp` from `MM.mm.pp[+build]`.
pub fn encode_fw_version(v: &str) -> Option<u32> {
    let semver = v.split('+').next()?;
    let n: Vec<u32> = semver.split('.').map(|p| p.parse().ok().filter(|x| *x < 256)).collect::<Option<_>>()?;
    let [a, b, c] = n[..] else { return None };
    Some(a << 16 | b << 8 | c)
}

/// Version of a DSP bundle: the image header's `fw_version` as `major.minor.patch` (bytes 2, 1, 0), or the
/// body CRC when the build left it at 0.
pub fn dsp_version(b: &DspBundle) -> String {
    match b.app.header.fw_version {
        0 => format!("build {:08x}", b.app.header.body_crc),
        v => format!("{}.{}.{}", (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        let s = FirmwareSources::from_dir(Path::new("rel"));
        assert_eq!(s.boot_build, Path::new("rel").join("boot"));
        let d = FirmwareSources::dev_tree(Path::new("oc"));
        assert!(d.tas_data.ends_with("oc_tas_data.bin"));
        assert!(d.backups.ends_with("backups"));
        assert!(s.dsp_bundle(None, vec![]).unwrap_err().contains("updater.out"));
    }

    #[test]
    fn targets_are_read_again_on_every_check() {
        use super::super::flow::{Chip, Flow};
        let dir = std::env::temp_dir().join(format!("oc-target-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = FirmwareSources::from_dir(&dir);
        std::fs::write(&s.dsp_app, b"..DOC0.2.5+1U..").unwrap();
        assert_eq!(s.target_version(&Flow::Update, Chip::Dsp).as_deref(), Some("0.2.5+1"));
        // Replaced while the app runs: the next check sees the new build.
        std::fs::write(&s.dsp_app, b"..DOC0.2.6+2U..").unwrap();
        assert_eq!(s.target_version(&Flow::Update, Chip::Dsp).as_deref(), Some("0.2.6+2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn versions_from_the_elf() {
        let mut elf = vec![0x7f];
        elf.extend_from_slice(b"ELF..DOC sth DOCx.yU..");
        elf.push(0);
        elf.extend_from_slice(b"DOC0.1.0+20261101183000U");
        elf.extend_from_slice(&[0, b'm']);
        assert_eq!(elf_version(&elf).as_deref(), Some("0.1.0+20261101183000"));
        assert_eq!(elf_version(b"DOC0.0.1U"), Some("0.0.1".into()));
        assert_eq!(elf_version(b"nothing"), None);
        assert_eq!(encode_fw_version("0.1.2+77"), Some(0x0000_0102));
        assert_eq!(encode_fw_version("1.2"), None);
    }
}
