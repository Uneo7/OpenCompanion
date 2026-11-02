//! `.ocprofile` files: one profile exported from (or imported into) a speaker slot.
//!
//! The file is JSON, readable and diffable, with the fields of `profile_t` (616 bytes, [`super::layout`]) as
//! structured values, in the same shape as `opencompanion/profiles/bose_stock.json` (which imports as is):
//!
//! ```json
//! {
//!   "format": "opencompanion-profile",
//!   "file_version": 1,
//!   "profile_version": 1,
//!   "size": 616,
//!   "generator": "OpenCompanion 0.1.0",
//!   "version": 1, "flags": 6, "flags_names": ["PF_LINK_LR", "PF_LOUDNESS"], "color": 3, "name": "Bureau",
//!   "sub_level_cdb": 0, "crossover_hz": 0, "aux_gain_cdb": 0, "trim_cdb": [0, 0, 0, 0, 0], "bass_milli": 0,
//!   "band_targets": ["global", "inner_L", "inner_R", "outer_L", "outer_R", "sub"],
//!   "band": [[{ "type": "peak", "reserved": 0, "freq_hz": 1000, "gain_cdb": -300, "q_milli": 1410 }, …], …],
//!   "bytes_hex": "0106…"
//! }
//! ```
//!
//! - `file_version` is this file layout (absent = 1); `profile_version` / `size` are `PROFILE_VERSION` /
//!   `PROFILE_SIZE` of the firmware the profile was made for.
//! - `band` is 6 targets × 12 slots, in [`super::EqTarget`] order. `type` is the `BAND_*` name in lower case
//!   (`off`, `peak`, `low_shelf`, `high_shelf`, `hpf`, `lpf`, `notch`; `raw` / `raw_data` of older files import as off); a raw biquad pair keeps
//!   its packed coefficients in `reserved` / `freq_hz` / `gain_cdb` / `q_milli`, exactly as stored.
//! - `flags_names`, `band_targets` and any other field are informative and ignored on import.
//! - `bytes_hex`, when present, is the 616 bytes of the profile: it must match the fields.
//!
//! Import validates everything: format, versions, shape, names, and every value against the firmware's limits
//! (`params_sanitize()`): a file the speaker would silently change is refused rather than altered.

use serde::{Deserialize, Serialize};

use super::band::{BandKind, BandSlot};
use super::layout::{flags, EQ_MAX_BANDS, EQ_OUTPUTS, EQ_TARGETS, NAME_LEN, PROFILE_SIZE, PROFILE_VERSION};
use super::{name_to_bytes, Profile};

pub const FORMAT: &str = "opencompanion-profile";
pub const FILE_VERSION: u32 = 1;
pub const EXTENSION: &str = "ocprofile";

const TARGET_NAMES: [&str; EQ_TARGETS] = ["global", "inner_L", "inner_R", "outer_L", "outer_R", "sub"];

const FLAG_NAMES: [(u8, &str); 5] = [
    (flags::BYPASS, "PF_BYPASS"),
    (flags::LINK_LR, "PF_LINK_LR"),
    (flags::LOUDNESS, "PF_LOUDNESS"),
    (flags::VOL_STOCK, "PF_VOL_STOCK"),
    (flags::STOCK_GRAPH, "PF_STOCK_GRAPH"),
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("not a readable profile file: {0}")]
    Json(String),
    #[error("not an OpenCompanion profile")]
    Format,
    #[error("file version {0} is not supported (expected {FILE_VERSION})")]
    FileVersion(u32),
    #[error("profile version {0} is not supported (expected {PROFILE_VERSION})")]
    ProfileVersion(u8),
    #[error("profile size {0} is not supported (expected {PROFILE_SIZE})")]
    Size(usize),
    #[error("{0}")]
    Shape(String),
    #[error("unknown band type \"{0}\"")]
    BandType(String),
    #[error("name longer than {NAME_LEN} bytes")]
    Name,
    #[error("{0} out of range")]
    Range(String),
    #[error("bytes_hex does not match the profile fields")]
    Bytes,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileBand {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    reserved: u8,
    freq_hz: u16,
    gain_cdb: i16,
    q_milli: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProfileFile {
    format: String,
    #[serde(default = "first_file_version")]
    file_version: u32,
    profile_version: u8,
    size: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    generator: Option<String>,
    version: u8,
    flags: u8,
    #[serde(default)]
    flags_names: Vec<String>,
    color: u8,
    #[serde(default)]
    reserved0: u8,
    name: String,
    sub_level_cdb: i16,
    crossover_hz: u16,
    aux_gain_cdb: i16,
    trim_cdb: Vec<i16>,
    #[serde(default)]
    bass_milli: i16,
    #[serde(default)]
    reserved1: [u8; 2],
    #[serde(default)]
    band_targets: Vec<String>,
    band: Vec<Vec<FileBand>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bytes_hex: Option<String>,
}

fn first_file_version() -> u32 {
    1
}

fn kind_name(k: BandKind) -> &'static str {
    match k {
        BandKind::Off => "off",
        BandKind::Peak => "peak",
        BandKind::LowShelf => "low_shelf",
        BandKind::HighShelf => "high_shelf",
        BandKind::HighPass => "hpf",
        BandKind::LowPass => "lpf",
        BandKind::Notch => "notch",
    }
}

fn kind_from_name(s: &str) -> Option<BandKind> {
    BandKind::ALL.into_iter().find(|k| kind_name(*k) == s)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

pub fn to_json(p: &Profile, generator: &str) -> String {
    let band = p
        .bands
        .iter()
        .map(|t| {
            t.iter()
                .map(|s| FileBand {
                    kind: kind_name(s.band_kind().unwrap_or(BandKind::Off)).into(),
                    reserved: s.reserved,
                    freq_hz: s.freq_hz,
                    gain_cdb: s.gain_cdb,
                    q_milli: s.q_milli,
                })
                .collect()
        })
        .collect();
    let f = ProfileFile {
        format: FORMAT.into(),
        file_version: FILE_VERSION,
        profile_version: PROFILE_VERSION,
        size: PROFILE_SIZE,
        generator: Some(generator.into()),
        version: p.version,
        flags: p.flags,
        flags_names: FLAG_NAMES.iter().filter(|(f, _)| p.has_flag(*f)).map(|(_, n)| (*n).into()).collect(),
        color: p.color,
        reserved0: p.reserved0,
        name: p.name(),
        sub_level_cdb: p.sub_level_cdb,
        crossover_hz: p.crossover_hz,
        aux_gain_cdb: p.aux_gain_cdb,
        trim_cdb: p.trim_cdb.to_vec(),
        bass_milli: p.bass_milli,
        reserved1: p.reserved1,
        band_targets: TARGET_NAMES.iter().map(|s| (*s).into()).collect(),
        band,
        bytes_hex: Some(hex(&p.to_bytes())),
    };
    serde_json::to_string_pretty(&f).expect("a profile always serializes")
}

pub fn from_json(json: &str) -> Result<Profile, FileError> {
    let f: ProfileFile = serde_json::from_str(json).map_err(|e| FileError::Json(e.to_string()))?;
    if f.format != FORMAT {
        return Err(FileError::Format);
    }
    if f.file_version != FILE_VERSION {
        return Err(FileError::FileVersion(f.file_version));
    }
    if f.profile_version != PROFILE_VERSION || f.version != PROFILE_VERSION {
        return Err(FileError::ProfileVersion(if f.profile_version != PROFILE_VERSION { f.profile_version } else { f.version }));
    }
    if f.size != PROFILE_SIZE {
        return Err(FileError::Size(f.size));
    }
    let trim_cdb: [i16; EQ_OUTPUTS] =
        f.trim_cdb.as_slice().try_into().map_err(|_| FileError::Shape(format!("trim_cdb needs {EQ_OUTPUTS} values")))?;
    if f.band.len() != EQ_TARGETS || f.band.iter().any(|t| t.len() != EQ_MAX_BANDS) {
        return Err(FileError::Shape(format!("band needs {EQ_TARGETS} targets of {EQ_MAX_BANDS} slots")));
    }
    if f.name.len() > NAME_LEN {
        return Err(FileError::Name);
    }
    let mut p = Profile {
        version: f.version,
        flags: f.flags,
        color: f.color,
        reserved0: f.reserved0,
        name: name_to_bytes(&f.name),
        sub_level_cdb: f.sub_level_cdb,
        crossover_hz: f.crossover_hz,
        aux_gain_cdb: f.aux_gain_cdb,
        trim_cdb,
        bass_milli: f.bass_milli,
        reserved1: f.reserved1,
        bands: Profile::defaults().bands,
    };
    let mut raw = false;
    for (target, slots) in p.bands.iter_mut().zip(&f.band) {
        for (slot, b) in target.iter_mut().zip(slots) {
            if b.kind == "raw" || b.kind == "raw_data" {
                // Raw biquad pairs (DSP profile protocol 6: removed): the slot is turned off, as the firmware does.
                *slot = BandSlot::off_at(slot.freq_hz);
                raw = true;
                continue;
            }
            let kind = kind_from_name(&b.kind).ok_or_else(|| FileError::BandType(b.kind.clone()))?;
            *slot = BandSlot { kind: kind.byte(), reserved: b.reserved, freq_hz: b.freq_hz, gain_cdb: b.gain_cdb, q_milli: b.q_milli };
        }
    }
    check_ranges(&p)?;
    if let (Some(h), false) = (&f.bytes_hex, raw) {
        if unhex(h).as_deref() != Some(&p.to_bytes()[..]) {
            return Err(FileError::Bytes);
        }
    }
    Ok(clear_reserved(p))
}

/// Fields reserved since DSP profile protocol 5 (`crossover_hz`, `bass_milli`): files written before keep
/// importing; the values are dropped (the firmware writes 0).
fn clear_reserved(mut p: Profile) -> Profile {
    p.crossover_hz = 0;
    p.bass_milli = 0;
    p
}

/// Refuses any value the firmware would change (`params_sanitize()`), naming the first one.
fn check_ranges(p: &Profile) -> Result<(), FileError> {
    let s = p.clone().sanitized();
    if s == clear_reserved(p.clone()) {
        return Ok(());
    }
    let field = |name: &str| Err(FileError::Range(name.into()));
    if s.flags != p.flags {
        return field("flags");
    }
    if (s.reserved0, s.reserved1) != (p.reserved0, p.reserved1) {
        return field("reserved");
    }
    if s.sub_level_cdb != p.sub_level_cdb {
        return field("sub_level_cdb");
    }
    if s.aux_gain_cdb != p.aux_gain_cdb {
        return field("aux_gain_cdb");
    }
    if s.trim_cdb != p.trim_cdb {
        return field("trim_cdb");
    }
    for (t, (a, b)) in s.bands.iter().zip(&p.bands).enumerate() {
        if let Some(k) = (0..EQ_MAX_BANDS).find(|k| a[*k] != b[*k]) {
            return field(&format!("band {} slot {}", TARGET_NAMES[t], k + 1));
        }
    }
    field("profile")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::EqTarget;

    const BOSE_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../profiles/bose_stock.json"));

    fn sample() -> Profile {
        let mut p = Profile::defaults();
        p.set_name("Bureau du soir");
        p.color = 3;
        p.set_flag(flags::LOUDNESS, true);
        p.sub_level_cdb = -250;
        p.trim_cdb = [0, -100, 50, 0, 600];
        p.target_mut(EqTarget::Global)[0] = BandSlot::new(BandKind::Peak, 1000, -300, 1410);
        p.target_mut(EqTarget::Sub)[11] = BandSlot::new(BandKind::HighPass, 25, 0, 707);
        p
    }

    #[test]
    fn round_trip() {
        let p = sample();
        let json = to_json(&p, "test");
        assert!(json.contains("\"format\": \"opencompanion-profile\"") && json.contains("\"type\": \"hpf\""));
        assert_eq!(from_json(&json), Ok(p));
    }

    #[test]
    fn the_bundled_bose_profile_imports() {
        let p = from_json(BOSE_JSON).expect("bose_stock.json");
        assert_eq!(p.name(), "Bose d'origine");
        assert!(p.has_flag(flags::STOCK_GRAPH) && p.bands.iter().flatten().all(|b| b.is_off()));
        assert_eq!(from_json(&to_json(&p, "test")), Ok(p));
    }

    fn edit(f: impl FnOnce(&mut serde_json::Value)) -> Result<Profile, FileError> {
        let mut v: serde_json::Value = serde_json::from_str(&to_json(&sample(), "test")).unwrap();
        f(&mut v);
        from_json(&v.to_string())
    }

    #[test]
    fn invalid_files_are_refused() {
        assert!(matches!(from_json("{"), Err(FileError::Json(_))));
        assert_eq!(edit(|v| v["format"] = "other".into()), Err(FileError::Format));
        assert_eq!(edit(|v| v["file_version"] = 2.into()), Err(FileError::FileVersion(2)));
        assert_eq!(edit(|v| v["profile_version"] = 2.into()), Err(FileError::ProfileVersion(2)));
        assert_eq!(edit(|v| v["size"] = 600.into()), Err(FileError::Size(600)));
        assert!(matches!(edit(|v| v["trim_cdb"] = serde_json::json!([0, 0])), Err(FileError::Shape(_))));
        assert!(matches!(edit(|v| v["band"].as_array_mut().unwrap().pop().map(drop).unwrap()), Err(FileError::Shape(_))));
        assert_eq!(edit(|v| v["band"][0][0]["type"] = "bell".into()), Err(FileError::BandType("bell".into())));
        assert_eq!(edit(|v| v["name"] = "a name far too long".into()), Err(FileError::Name));
        assert_eq!(edit(|v| v["sub_level_cdb"] = 5000.into()), Err(FileError::Range("sub_level_cdb".into())));
        assert_eq!(edit(|v| v["flags"] = 0x80.into()), Err(FileError::Range("flags".into())));
        assert_eq!(edit(|v| v["band"][0][0]["gain_cdb"] = 9000.into()), Err(FileError::Range("band global slot 1".into())));
        assert_eq!(edit(|v| v["color"] = 4.into()), Err(FileError::Bytes));
        // Without bytes_hex the fields alone are enough.
        let p = edit(|v| {
            v.as_object_mut().unwrap().remove("bytes_hex");
            v["color"] = 4.into();
        })
        .unwrap();
        assert_eq!(p.color, 4);
    }
}
