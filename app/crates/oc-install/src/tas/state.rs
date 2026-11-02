use oc_tas_image::{DataType, BOSE_HEADER};

use super::target::{header_shape_ok, parse_header, TasTarget, DATA_TYPE_AT, HEADER_LEN};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TasContent {
    Target,
    /// Firmware 01.01.03, DataType 1.
    Bose,
    /// DataType 3: an install was interrupted. Still running from RAM if no power cycle happened since;
    /// after a power cycle the ROM boots its DFU.
    InProgress,
    /// DataType 2: the ROM boots its DFU (developer mode).
    DfuMode,
    OtherApplication { firmware_version: u8, code_len: usize },
    Invalid,
}

pub fn classify(ee: &[u8], target: Option<&TasTarget>) -> TasContent {
    let same = |t: &TasTarget| {
        ee.len() == t.bytes.len() && ee.iter().zip(&t.bytes).enumerate().all(|(i, (a, b))| a == b || i == super::plan::MODE_ADDR)
    };
    if target.is_some_and(same) {
        return TasContent::Target;
    }
    let h = &ee[..HEADER_LEN.min(ee.len())];
    if !header_shape_ok(h) {
        return TasContent::Invalid;
    }
    match DataType::from(h[DATA_TYPE_AT]) {
        DataType::Updating => return TasContent::InProgress,
        DataType::Device => return TasContent::DfuMode,
        _ => {}
    }
    if h == BOSE_HEADER {
        return TasContent::Bose;
    }
    match parse_header(h) {
        Some(p) if p.data_type == DataType::Application => {
            TasContent::OtherApplication { firmware_version: p.firmware_version, code_len: p.payload_size as usize }
        }
        _ => TasContent::Invalid,
    }
}
