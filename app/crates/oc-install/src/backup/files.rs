use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use oc_ais::eeprom::header::ImageHeader;
use oc_ais::eeprom::layout::{BOOT1, EE_PAGE, EE_SIZE, HDR};
use oc_ais::MAGIC;
use oc_tas_image::BOSE_HEADER;

use crate::dsp::bundle::OP_DISABLE_CRC;
use crate::error::{Error, Result};

pub const TAS_FILE: &str = "u6002_tas.bin";

/// Any stock Bose USB controller image (whatever its revision): an application header for 05A7:1020, without
/// the OpenCompanion data area. With only the header: the header checks.
pub fn is_stock_tas(b: &[u8]) -> bool {
    use oc_tas_image::{data_area, DataType, Header};
    const DATA_AREA: usize = 0x1800;
    let Ok(h) = Header::parse(b) else { return false };
    h.data_type == DataType::Application
        && (h.vendor_id, h.product_id) == (0x05A7, 0x1020)
        && b.get(DATA_AREA..DATA_AREA + 4) != Some(&data_area::MAGIC[..])
}
/// The stock firmware's `SVM` answer when the backup was read (e.g. `D01.01.03U01.01.04`).
pub const VERSION_FILE: &str = "firmware-version.txt";
pub const DSP_FILE: &str = "u7001_dsp.bin";
pub const SUMS_FILE: &str = "checksums.txt";
const TAS_SIZE: usize = crate::tas::EE_SIZE;

pub fn default_root() -> PathBuf {
    crate::paths::backups_dir()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    Stock,
    OpenCompanion,
    Other,
}

impl Content {
    fn word(self) -> &'static str {
        match self {
            Self::Stock => "stock",
            Self::OpenCompanion => "opencompanion",
            Self::Other => "other",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "stock" => Self::Stock,
            "opencompanion" => Self::OpenCompanion,
            _ => Self::Other,
        }
    }

    pub fn of_tas(b: &[u8]) -> Self {
        match b.get(..BOSE_HEADER.len()) {
            Some(h) if h == BOSE_HEADER || is_stock_tas(b) => Self::Stock,
            _ if b.len() > 3 && b[1] == 0x12 && b[2] == 0x12 && b[3] == 0x34 => Self::OpenCompanion,
            _ => Self::Other,
        }
    }

    pub fn of_dsp(b: &[u8]) -> Self {
        let le32 = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        if b.len() != EE_SIZE || le32(0) != MAGIC {
            return Self::Other;
        }
        if le32(4) == OP_DISABLE_CRC
            && ImageHeader::parse(&b[HDR..HDR + EE_PAGE]).is_err()
            && oc_ais::parse(&b[..BOOT1.start]).is_ok_and(|a| a.entry().is_some())
        {
            Self::Stock
        } else if le32(4) == 0x5853_5901 {
            Self::OpenCompanion
        } else {
            Self::Other
        }
    }
}

impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dump {
    pub crc32: u32,
    pub content: Content,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub dir: PathBuf,
    pub speaker: String,
    /// `YYYY-MM-DD_HHMMSS` (UTC).
    pub stamp: String,
    pub tas: Option<Dump>,
    pub dsp: Option<Dump>,
}

impl BackupEntry {
    pub fn is_stock(&self) -> bool {
        self.tas.is_some_and(|d| d.content == Content::Stock) && self.dsp.is_some_and(|d| d.content == Content::Stock)
    }

    pub fn date(&self) -> &str {
        self.stamp.get(..10).unwrap_or(&self.stamp)
    }
}

#[derive(Debug, Clone)]
pub struct BackupFiles {
    pub entry: BackupEntry,
    pub tas: Option<Vec<u8>>,
    pub dsp: Option<Vec<u8>>,
    /// The stock firmware's `SVM` answer when the backup was read ([`VERSION_FILE`]; older backups: none).
    pub firmware: Option<String>,
}

fn io(what: &str, p: &Path, e: std::io::Error) -> Error {
    Error::File(format!("{what} {}: {e}", p.display()))
}

fn write_new(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).map_err(|e| io("writing", &tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| io("renaming", &tmp, e))?;
    if fs::read(path).map_err(|e| io("reading back", path, e))? != data {
        return Err(Error::File(format!("{} differs after writing", path.display())));
    }
    Ok(())
}

/// Refuses to touch an existing folder.
pub fn save(root: &Path, speaker: &str, stamp: &str, tas: Option<&[u8]>, dsp: Option<&[u8]>) -> Result<BackupEntry> {
    if tas.is_none() && dsp.is_none() {
        return Err(Error::File("nothing to save".into()));
    }
    let dir = root.join(speaker).join(stamp);
    if dir.exists() {
        return Err(Error::File(format!("{} already exists: backups are never overwritten", dir.display())));
    }
    fs::create_dir_all(&dir).map_err(|e| io("creating", &dir, e))?;
    let mut sums = format!("# OpenCompanion EEPROM backup, read from the speaker\n# speaker {speaker}\n# taken {stamp} UTC\n");
    let mut entry = BackupEntry { dir: dir.clone(), speaker: speaker.into(), stamp: stamp.into(), tas: None, dsp: None };
    for (name, data, slot, content) in [
        (TAS_FILE, tas, &mut entry.tas, tas.map(Content::of_tas)),
        (DSP_FILE, dsp, &mut entry.dsp, dsp.map(Content::of_dsp)),
    ] {
        let (Some(data), Some(content)) = (data, content) else { continue };
        write_new(&dir.join(name), data)?;
        let crc32 = crc32fast::hash(data);
        sums.push_str(&format!("0x{crc32:08x} {} {name} {content}\n", data.len()));
        *slot = Some(Dump { crc32, content });
    }
    write_new(&dir.join(SUMS_FILE), sums.as_bytes())?; // last: marks the folder complete
    Ok(entry)
}

pub fn load(dir: &Path) -> Result<BackupFiles> {
    let sums_path = dir.join(SUMS_FILE);
    let sums = fs::read_to_string(&sums_path).map_err(|e| io("reading", &sums_path, e))?;
    let name = |p: &Path| p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let mut out = BackupFiles {
        entry: BackupEntry {
            dir: dir.to_path_buf(),
            speaker: dir.parent().map(name).unwrap_or_default(),
            stamp: name(dir),
            tas: None,
            dsp: None,
        },
        tas: None,
        dsp: None,
        firmware: fs::read_to_string(dir.join(VERSION_FILE)).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()),
    };
    for line in sums.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let bad = || Error::File(format!("{}: bad line {line:?}", sums_path.display()));
        let [crc, size, file, content] = f[..] else { return Err(bad()) };
        let crc = u32::from_str_radix(crc.trim_start_matches("0x"), 16).map_err(|_| bad())?;
        let size: usize = size.parse().map_err(|_| bad())?;
        let (want, slot, data_slot) = match file {
            TAS_FILE => (TAS_SIZE, &mut out.entry.tas, &mut out.tas),
            DSP_FILE => (EE_SIZE, &mut out.entry.dsp, &mut out.dsp),
            _ => return Err(bad()),
        };
        let path = dir.join(file);
        let data = fs::read(&path).map_err(|e| io("reading", &path, e))?;
        if size != want || data.len() != size || crc32fast::hash(&data) != crc {
            return Err(Error::Verify(format!("{} does not match {SUMS_FILE} (size or CRC-32)", path.display())));
        }
        *slot = Some(Dump { crc32: crc, content: Content::parse(content) });
        *data_slot = Some(data);
    }
    if out.tas.is_none() && out.dsp.is_none() {
        return Err(Error::File(format!("{} lists no dump", sums_path.display())));
    }
    Ok(out)
}

/// Oldest first (per speaker, then by stamp).
pub fn list(root: &Path) -> Vec<BackupEntry> {
    let subdirs = |p: &Path| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = fs::read_dir(p)
            .map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect())
            .unwrap_or_default();
        v.sort();
        v
    };
    subdirs(root)
        .iter()
        .flat_map(|s| subdirs(s))
        .filter(|d| d.join(SUMS_FILE).is_file())
        .filter_map(|d| load(&d).ok().map(|f| f.entry))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("oc-backup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn any_stock_tas_revision_is_stock() {
        let mut h = oc_tas_image::Header::parse(&BOSE_HEADER).unwrap();
        h.firmware_version = 0x05; // a newer Bose soft update (01.01.04)
        let mut ee = h.to_bytes();
        ee.resize(8192, 0xFF);
        assert!(is_stock_tas(&ee) && Content::of_tas(&ee) == Content::Stock);
        ee[0x1800..0x1804].copy_from_slice(&oc_tas_image::data_area::MAGIC);
        assert!(!is_stock_tas(&ee), "our data area: not stock");
        h.data_type = oc_tas_image::DataType::Device;
        assert!(!is_stock_tas(&h.to_bytes()));
    }

    #[test]
    fn save_load_list_and_never_overwrite() {
        let root = tmp("files");
        let tas = vec![0x5Au8; TAS_SIZE];
        let e = save(&root, "c5-0000", "2026-11-01_100000", Some(&tas), None).unwrap();
        assert_eq!(e.tas.unwrap().crc32, crc32fast::hash(&tas));
        assert!(save(&root, "c5-0000", "2026-11-01_100000", Some(&tas), None).is_err(), "never overwritten");
        save(&root, "c5-0000", "2026-11-02_100000", Some(&tas), None).unwrap();
        let l = list(&root);
        assert_eq!(l.iter().map(|e| e.stamp.as_str()).collect::<Vec<_>>(), ["2026-11-01_100000", "2026-11-02_100000"]);
        assert_eq!(load(&e.dir).unwrap().tas.unwrap(), tas);
        fs::write(e.dir.join(TAS_FILE), vec![0u8; TAS_SIZE]).unwrap();
        assert!(matches!(load(&e.dir), Err(Error::Verify(_))));
        assert_eq!(list(&root).len(), 1);
        fs::create_dir_all(root.join("c5-0000/2026-11-03_100000")).unwrap();
        assert_eq!(list(&root).len(), 1);
        let _ = fs::remove_dir_all(&root);
    }
}
