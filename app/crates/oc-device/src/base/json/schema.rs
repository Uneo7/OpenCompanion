//! Unknown keys are refused, except `about` (annotation, ignored on import) on every object.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT: &str = "opencompanion-base";
pub const VERSION: u32 = 1;

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileJson {
    pub format: String,
    pub version: u32,
    pub topology: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceJson>,
    pub sample_rate: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
    pub aux_mix: AuxMixJson,
    pub input_gain: [f64; 6],
    pub centre_mix: CentreMixJson,
    pub surround_mix: SurroundMixJson,
    pub bass_sum: BassSumJson,
    pub driver_eq: DriverEqJson,
    pub bass_phase: SetJson,
    pub surround_net: SurroundNetJson,
    pub front_net: FrontNetJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceJson {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    /// `"0x…"`, CRC-32 of the 64 KB backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_crc32: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuxMixJson {
    pub usb: [f64; 2],
    pub aux: [f64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CentreMixJson {
    pub direct: [f64; 2],
    pub centre: [f64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurroundMixJson {
    pub surround: [f64; 2],
    pub third: [f64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BassSumJson {
    pub direct: [f64; 2],
    pub array: [f64; 2],
    pub surround: [f64; 2],
    pub lfe: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverEqJson {
    pub direct: SetJson,
    pub array: SetJson,
    pub surround: SetJson,
    pub bass: SetJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurroundNetJson {
    pub delay_in: [u8; 2],
    pub pre: SetJson,
    pub stage1: SetJson,
    pub cross1: [u8; 2],
    pub stage2: SetJson,
    pub cross2: [u8; 2],
    pub stage3: SetJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontNetJson {
    pub direct: SetJson,
    pub array_a: SetJson,
    pub array_x: SetJson,
    pub stage_b: SetJson,
    pub stage_b_x: SetJson,
    pub delay: [u8; 2],
    pub stage_c: SetJson,
    pub stage_c_x: SetJson,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

/// A set: exactly one of `gain` (linear) or `gain_db` (+ `invert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_db: Option<f64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
    pub sections: Vec<SectionJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityTag {
    #[serde(rename = "identity")]
    Identity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SectionJson {
    Identity(IdentityTag),
    Form(Box<SectionObj>),
}

/// One section: exactly one form key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionObj {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zpk: Option<ZpkJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_shelf: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_shelf: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lowpass: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highpass: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandpass: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notch: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allpass: Option<RbjJson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZpkJson {
    pub zeros: RootsJson,
    pub poles: RootsJson,
}

/// A conjugate pair (`f_hz`, `r`) or two real roots (`real`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootsJson {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RbjJson {
    pub f_hz: f64,
    pub q: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_db: Option<f64>,
}
