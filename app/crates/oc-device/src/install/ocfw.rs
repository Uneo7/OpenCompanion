//! `.ocfw`: the OpenCompanion firmware container, one file per chip. The app installs firmware files only in this
//! form: the header is checked first (magic, chip, size, CRC-32, minimum app version), then the files inside go
//! through the identity checks of [`super::identity`] before anything is written.
//!
//! Layout (little-endian):
//!
//! | Offset | Size | Field |
//! |---|---|---|
//! | 0 | 4 | magic `"OCFW"` |
//! | 4 | 2 | format version (1) |
//! | 6 | 1 | chip: 1 = DSP, 2 = USB controller |
//! | 7 | 1 | reserved (0) |
//! | 8 | 4 | payload length |
//! | 12 | 4 | CRC-32 of the payload |
//! | 16 | 32 | firmware version, UTF-8, NUL-padded (`0.2.1+20261101235107`, `0.4.9`) |
//! | 48 | 16 | minimum app version, UTF-8, NUL-padded |
//! | 64 | … | payload: entry count (u16), then per entry: name length (u16), name (UTF-8, `/` paths), data length (u32), data |
//!
//! Entries: DSP `opencompanion_dsp.out`, `loader.out`, `boot/tramp.out`, `boot/boot1.out`, `boot/updater.out`;
//! USB `oc_tas.dfu`, `oc_tas_data.bin` (the layout of [`super::FirmwareSources::from_dir`]).

use std::path::{Path, PathBuf};

use super::flow::Chip;

pub const MAGIC: [u8; 4] = *b"OCFW";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_LEN: usize = 64;
pub const EXTENSION: &str = "ocfw";
/// File names in a release folder.
pub const DSP_FILE: &str = "opencompanion-dsp.ocfw";
pub const USB_FILE: &str = "opencompanion-usb.ocfw";

pub const DSP_ENTRIES: [&str; 5] = ["opencompanion_dsp.out", "loader.out", "boot/tramp.out", "boot/boot1.out", "boot/updater.out"];
pub const USB_ENTRIES: [&str; 2] = ["oc_tas.dfu", "oc_tas_data.bin"];

/// The files a container of `chip` carries.
pub fn entries_of(chip: Chip) -> &'static [&'static str] {
    match chip {
        Chip::Dsp => &DSP_ENTRIES,
        Chip::Usb => &USB_ENTRIES,
    }
}

fn chip_byte(c: Chip) -> u8 {
    match c {
        Chip::Dsp => 1,
        Chip::Usb => 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ocfw {
    pub chip: Chip,
    pub version: String,
    pub min_app_version: String,
    /// `(name, data)`, in [`entries_of`] order.
    pub entries: Vec<(String, Vec<u8>)>,
}

fn put_str(out: &mut [u8], s: &str) -> Result<(), String> {
    if s.len() > out.len() {
        return Err(format!("\"{s}\" longer than {} bytes", out.len()));
    }
    out[..s.len()].copy_from_slice(s.as_bytes());
    Ok(())
}

fn get_str(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// `a` < `b` on `major.minor.patch` (anything after `+` or `-` ignored).
pub fn semver_lt(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        let v = v.trim().trim_start_matches("OC-").trim_start_matches("OC");
        v.split(['+', '-']).next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    parse(a) < parse(b)
}

impl Ocfw {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut payload = (self.entries.len() as u16).to_le_bytes().to_vec();
        for (name, data) in &self.entries {
            payload.extend((name.len() as u16).to_le_bytes());
            payload.extend(name.as_bytes());
            payload.extend((data.len() as u32).to_le_bytes());
            payload.extend(data);
        }
        let mut h = vec![0u8; HEADER_LEN];
        h[..4].copy_from_slice(&MAGIC);
        h[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        h[6] = chip_byte(self.chip);
        h[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        h[12..16].copy_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        put_str(&mut h[16..48], &self.version)?;
        put_str(&mut h[48..64], &self.min_app_version)?;
        h.extend(payload);
        Ok(h)
    }

    /// Checks and opens a container: magic, format, chip (`expected`), length, CRC-32, the entries of that chip,
    /// and that this app is recent enough (`app_version`).
    pub fn parse(b: &[u8], expected: Chip, app_version: &str) -> Result<Self, String> {
        if b.len() < HEADER_LEN || b[..4] != MAGIC {
            return Err("not an OpenCompanion firmware file (.ocfw)".into());
        }
        let fmt = u16::from_le_bytes([b[4], b[5]]);
        if fmt != FORMAT_VERSION {
            return Err(format!("firmware file format {fmt} is not supported"));
        }
        let chip = match b[6] {
            1 => Chip::Dsp,
            2 => Chip::Usb,
            c => return Err(format!("unknown chip {c} in the firmware file")),
        };
        if chip != expected {
            return Err(format!("this firmware is for the {}, not the {}", name(chip), name(expected)));
        }
        let len = u32::from_le_bytes(b[8..12].try_into().expect("4 bytes")) as usize;
        let payload = b.get(HEADER_LEN..HEADER_LEN + len).ok_or("the firmware file is truncated")?;
        if b.len() != HEADER_LEN + len {
            return Err("the firmware file has trailing bytes".into());
        }
        if crc32fast::hash(payload) != u32::from_le_bytes(b[12..16].try_into().expect("4 bytes")) {
            return Err("the firmware file is damaged (CRC-32)".into());
        }
        let version = get_str(&b[16..48]);
        let min_app_version = get_str(&b[48..64]);
        if semver_lt(app_version, &min_app_version) {
            return Err(format!("this firmware needs OpenCompanion {min_app_version} or newer"));
        }
        let entries = parse_entries(payload)?;
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        if names != entries_of(chip) {
            return Err(format!("unexpected files in the firmware file: {names:?}"));
        }
        Ok(Self { chip, version, min_app_version, entries })
    }

    /// Writes the entries under `dir` (the layout of `FirmwareSources::from_dir`).
    pub fn extract(&self, dir: &Path) -> Result<(), String> {
        for (name, data) in &self.entries {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().expect("parent")).map_err(|e| e.to_string())?;
            std::fs::write(&p, data).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(())
    }

    /// Packs the files of `chip` from a folder in the release layout.
    pub fn from_dir(dir: &Path, chip: Chip, version: &str, min_app_version: &str) -> Result<Self, String> {
        let entries = entries_of(chip)
            .iter()
            .map(|n| std::fs::read(dir.join(n)).map(|d| ((*n).to_owned(), d)).map_err(|e| format!("{n}: {e}")))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { chip, version: version.into(), min_app_version: min_app_version.into(), entries })
    }
}

fn name(c: Chip) -> &'static str {
    match c {
        Chip::Dsp => "audio processor (DSP)",
        Chip::Usb => "USB controller",
    }
}

fn parse_entries(p: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let bad = || "the firmware file is malformed".to_owned();
    let n = u16::from_le_bytes([*p.first().ok_or_else(bad)?, *p.get(1).ok_or_else(bad)?]) as usize;
    let mut at = 2;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let nl = u16::from_le_bytes(p.get(at..at + 2).ok_or_else(bad)?.try_into().expect("2")) as usize;
        at += 2;
        let name = std::str::from_utf8(p.get(at..at + nl).ok_or_else(bad)?).map_err(|_| bad())?.to_owned();
        at += nl;
        let dl = u32::from_le_bytes(p.get(at..at + 4).ok_or_else(bad)?.try_into().expect("4")) as usize;
        at += 4;
        let data = p.get(at..at + dl).ok_or_else(bad)?.to_vec();
        at += dl;
        if name.contains("..") || name.starts_with('/') || name.contains('\\') {
            return Err(bad());
        }
        out.push((name, data));
    }
    if at != p.len() {
        return Err(bad());
    }
    Ok(out)
}

/// Opens the `.ocfw` at `path` for `chip` and extracts it into a fresh folder under the system temp dir; returns
/// the folder (release layout) and the container's version.
pub fn extract_file(path: &Path, chip: Chip, app_version: &str) -> Result<(PathBuf, String), String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let f = Ocfw::parse(&b, chip, app_version)?;
    let dir = std::env::temp_dir().join(format!("opencompanion-ocfw-{}-{}", chip_byte(chip), std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    f.extract(&dir)?;
    Ok((dir, f.version))
}

/// The version in the header of the `.ocfw` at `path` (no other check).
pub fn peek_version(path: &Path) -> Option<String> {
    let b = std::fs::read(path).ok()?;
    (b.len() >= HEADER_LEN && b[..4] == MAGIC).then(|| get_str(&b[16..48]))
}

/// A release folder holding [`DSP_FILE`] and [`USB_FILE`]: both extracted (checked) into `into`.
pub fn expand_release(dir: &Path, into: &Path, app_version: &str) -> Result<(String, String), String> {
    let open = |name: &str, chip: Chip| -> Result<Ocfw, String> {
        let b = std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
        Ocfw::parse(&b, chip, app_version).map_err(|e| format!("{name}: {e}"))
    };
    let (dsp, usb) = (open(DSP_FILE, Chip::Dsp)?, open(USB_FILE, Chip::Usb)?);
    dsp.extract(into)?;
    usb.extract(into)?;
    Ok((dsp.version, usb.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb() -> Ocfw {
        Ocfw {
            chip: Chip::Usb,
            version: "0.4.9".into(),
            min_app_version: "0.1.0".into(),
            entries: vec![("oc_tas.dfu".into(), vec![1, 2, 3]), ("oc_tas_data.bin".into(), vec![4; 40])],
        }
    }

    #[test]
    fn containers_are_checked_before_anything_else() {
        let b = usb().to_bytes().unwrap();
        assert_eq!(Ocfw::parse(&b, Chip::Usb, "0.1.0"), Ok(usb()));
        // Wrong magic, wrong chip, bad CRC, truncated, trailing bytes, app too old, random bytes.
        let mut m = b.clone();
        m[0] = b'X';
        assert!(Ocfw::parse(&m, Chip::Usb, "0.1.0").unwrap_err().contains("not an OpenCompanion"));
        assert!(Ocfw::parse(&b, Chip::Dsp, "0.1.0").unwrap_err().contains("not the audio processor"));
        let mut c = b.clone();
        let last = c.len() - 1;
        c[last] ^= 1;
        assert!(Ocfw::parse(&c, Chip::Usb, "0.1.0").unwrap_err().contains("CRC"));
        assert!(Ocfw::parse(&b[..b.len() - 5], Chip::Usb, "0.1.0").unwrap_err().contains("truncated"));
        assert!(Ocfw::parse(&[b.clone(), vec![0]].concat(), Chip::Usb, "0.1.0").is_err());
        assert!(Ocfw::parse(&b, Chip::Usb, "0.0.9").unwrap_err().contains("needs OpenCompanion 0.1.0"));
        assert!(Ocfw::parse(&[0x5a; 300], Chip::Usb, "0.1.0").is_err());
        // Missing entry.
        let mut short = usb();
        short.entries.pop();
        assert!(Ocfw::parse(&short.to_bytes().unwrap(), Chip::Usb, "0.1.0").unwrap_err().contains("unexpected files"));
        assert_eq!(peek_version_bytes(&b), Some("0.4.9".into()));
    }

    fn peek_version_bytes(b: &[u8]) -> Option<String> {
        (b.len() >= HEADER_LEN && b[..4] == MAGIC).then(|| get_str(&b[16..48]))
    }

    #[test]
    fn versions_compare() {
        assert!(semver_lt("0.4.9", "0.4.10") && semver_lt("OC0.1.0+5", "0.2.0") && !semver_lt("0.2.1+1", "0.2.1+0"));
    }
}
