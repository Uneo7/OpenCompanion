use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FORMAT: &str = "opencompanion-firmware";
pub const MANIFEST_VERSION: u32 = 1;
pub const MANIFEST_NAME: &str = "firmware-manifest.json";
/// Every file of a release: the two `.ocfw` containers (one per chip).
pub const FILES: [&str; 2] = [crate::install::ocfw::DSP_FILE, crate::install::ocfw::USB_FILE];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub manifest_version: u32,
    pub dsp_version: String,
    pub tas_version: String,
    pub min_app_version: String,
    pub files: Vec<FileEntry>,
}

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// `a` < `b` on `major.minor.patch` (anything after `+` or `-` ignored).
pub(crate) fn semver_lt(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split(['+', '-']).next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    parse(a) < parse(b)
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, String> {
        let m: Manifest = serde_json::from_str(json).map_err(|e| format!("manifest: {e}"))?;
        if m.format != FORMAT {
            return Err("manifest: not an OpenCompanion firmware manifest".into());
        }
        if m.manifest_version != MANIFEST_VERSION {
            return Err(format!("manifest version {} is not supported", m.manifest_version));
        }
        let mut paths: Vec<&str> = m.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort_unstable();
        let mut want = FILES.to_vec();
        want.sort_unstable();
        if paths != want {
            return Err(format!("manifest: files {paths:?}, expected {want:?}"));
        }
        if m.files.iter().any(|f| f.sha256.len() != 64 || !f.sha256.bytes().all(|c| c.is_ascii_hexdigit())) {
            return Err("manifest: bad sha256".into());
        }
        Ok(m)
    }

    pub fn app_supported(&self, app_version: &str) -> bool {
        !semver_lt(app_version, &self.min_app_version)
    }

    pub fn verify_dir(&self, dir: &Path) -> Result<(), String> {
        for f in &self.files {
            let p = dir.join(&f.path);
            let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", f.path))?;
            if data.len() as u64 != f.size {
                return Err(format!("{}: {} bytes, the manifest says {}", f.path, data.len(), f.size));
            }
            if sha256_hex(&data) != f.sha256.to_ascii_lowercase() {
                return Err(format!("{}: SHA-256 does not match the manifest", f.path));
            }
        }
        Ok(())
    }

    pub fn of_dir(dir: &Path, dsp_version: &str, tas_version: &str, min_app_version: &str) -> Result<Self, String> {
        let files = FILES
            .iter()
            .map(|path| {
                let data = std::fs::read(dir.join(path)).map_err(|e| format!("{path}: {e}"))?;
                Ok(FileEntry { path: (*path).into(), size: data.len() as u64, sha256: sha256_hex(&data) })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            format: FORMAT.into(),
            manifest_version: MANIFEST_VERSION,
            dsp_version: dsp_version.into(),
            tas_version: tas_version.into(),
            min_app_version: min_app_version.into(),
            files,
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a manifest always serializes")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A release folder with two small (valid) containers, and its manifest.
    pub fn fake_release(dir: &Path) -> Manifest {
        use crate::install::flow::Chip;
        use crate::install::ocfw::{entries_of, Ocfw, DSP_FILE, USB_FILE};
        std::fs::create_dir_all(dir).unwrap();
        for (chip, name) in [(Chip::Dsp, DSP_FILE), (Chip::Usb, USB_FILE)] {
            let entries = entries_of(chip).iter().map(|n| ((*n).to_owned(), n.as_bytes().to_vec())).collect();
            let f = Ocfw { chip, version: "0.1.0".into(), min_app_version: "0.1.0".into(), entries };
            std::fs::write(dir.join(name), f.to_bytes().unwrap()).unwrap();
        }
        Manifest::of_dir(dir, "0.1.0+20261102120000", "0.4.1", "0.1.0").unwrap()
    }

    #[test]
    fn manifests_round_trip_and_verify_files() {
        let dir = std::env::temp_dir().join(format!("oc-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let m = fake_release(&dir);
        let back = Manifest::parse(&m.to_json()).unwrap();
        assert_eq!(back, m);
        assert!(back.verify_dir(&dir).is_ok());
        std::fs::write(dir.join(crate::install::ocfw::USB_FILE), "changed!!").unwrap();
        assert!(back.verify_dir(&dir).unwrap_err().contains("usb"));
        assert!(m.app_supported("0.1.0") && m.app_supported("0.2.0") && !m.app_supported("0.0.9"));
        let mut bad = m.clone();
        bad.files.pop();
        assert!(Manifest::parse(&bad.to_json()).is_err());
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
