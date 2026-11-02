//! Reads only: nothing here writes RAM or EEPROM.

use std::fmt;
use std::time::Duration;

use oc_ais::eeprom::header::ImageHeader;
use oc_ais::eeprom::layout::{BOOT0, EE_PAGE, HDR};
use oc_protocol::oc_hid::{self, cmd, OcInfo, OcLink};
use oc_protocol::{tas_eeprom, Link};

use crate::dsp::{detect, DspRuntime, Updater};
use crate::tas::{classify, TasContent, HEADER_LEN};

/// USB identities of the TAS ROM DFU: invalid EEPROM, TI DFU identity, or the stock header VID:PID.
/// 05A7:1020 is also the stock firmware and OpenCompanion 0.5.0+: only meaningful when no vendor HID interface was
/// found (the ROM DFU has none), as `tas_without_hid` and `oc_device::link::detect` use it.
pub const TAS_DFU_IDS: [(u16, u16); 3] = [(0xFFFF, 0xFFFE), (0x0451, 0x1021), (0x05A7, 0x1020)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TasStatus {
    Bose { eeprom: TasContent },
    OpenCompanion { info: OcInfo, eeprom: TasContent },
    /// Interrupted TAS install (DataType 2/3): recovery needed.
    RomDfu { vid: u16, pid: u16 },
    NotFound,
    Error(String),
}

impl fmt::Display for TasStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bose { eeprom } => write!(f, "stock Bose TAS firmware running; U6002 header: {}", tas_text(eeprom)),
            Self::OpenCompanion { info, eeprom } => write!(
                f,
                "OpenCompanion TAS firmware {} (protocol {}){}; U6002 header: {}",
                info.version_string(),
                info.protocol,
                if info.ee_session() { ", EEPROM update session open" } else { "" },
                tas_text(eeprom)
            ),
            Self::RomDfu { vid, pid } => write!(
                f,
                "TAS ROM in DFU mode ({vid:04x}:{pid:04x}): an install was interrupted, see opencompanion/docs/recovery.md"
            ),
            Self::NotFound => write!(f, "no speaker found"),
            Self::Error(e) => write!(f, "TAS read error: {e}"),
        }
    }
}

fn tas_text(c: &TasContent) -> String {
    match c {
        TasContent::Target => "the selected image".into(),
        TasContent::Bose => "stock Bose (01.01.03)".into(),
        TasContent::InProgress => "DataType 3: install in progress (ROM DFU at the next power-up)".into(),
        TasContent::DfuMode => "DataType 2: ROM DFU at the next power-up".into(),
        TasContent::OtherApplication { firmware_version, code_len } => {
            format!("application, firmware version {firmware_version}, {code_len} bytes of code")
        }
        TasContent::Invalid => "no valid boot header".into(),
    }
}

pub fn tas_bose(link: &dyn Link) -> TasStatus {
    match tas_eeprom::read_block(link, 0, HEADER_LEN as u8) {
        Ok(h) => TasStatus::Bose { eeprom: classify(&h, None) },
        Err(e) => TasStatus::Error(e.to_string()),
    }
}

pub fn tas_oc(link: &dyn OcLink) -> TasStatus {
    let r = (|| {
        let info = oc_hid::info(link)?;
        let h = link.call(cmd::EE_READ, &[0, 0, HEADER_LEN as u8], Duration::from_millis(500))?;
        Ok::<_, oc_protocol::Error>(TasStatus::OpenCompanion { info, eeprom: classify(&h, None) })
    })();
    r.unwrap_or_else(|e| TasStatus::Error(e.to_string()))
}

/// `present` = USB VID:PIDs seen on the bus.
pub fn tas_without_hid(present: &[(u16, u16)]) -> TasStatus {
    match present.iter().find(|id| TAS_DFU_IDS.contains(id)) {
        Some(&(vid, pid)) => TasStatus::RomDfu { vid, pid },
        None => TasStatus::NotFound,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DspStatus {
    pub runtime: DspRuntime,
    /// Only when the updater already runs (reads through it): boot0 and header summary.
    pub eeprom: Option<String>,
}

impl fmt::Display for DspStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.runtime)?;
        match (&self.eeprom, &self.runtime) {
            (Some(e), _) => write!(f, "; U7001: {e}"),
            (None, DspRuntime::Stock { .. }) => write!(
                f,
                "; U7001 boots the stock image (migration steps M2 may be partly done: `oc install dsp --dry-run` reads it)"
            ),
            (None, DspRuntime::OcFirmware { .. }) => write!(
                f,
                "; running from U7001 or from a RAM load: U7001 content unknown (`oc install dsp --dry-run` reads it)"
            ),
            (None, DspRuntime::Rescue { .. }) => write!(f, "; U7001 boot chain installed, image missing or invalid"),
            _ => Ok(()),
        }
    }
}

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn updater_eeprom(u: &Updater) -> crate::Result<String> {
    let b0 = u.ee_read(BOOT0.start, 16)?;
    let boot0 = match (le32(&b0, 0), le32(&b0, 4)) {
        (oc_ais::MAGIC, 0x5853_5904) => "stock boot0",
        (oc_ais::MAGIC, 0x5853_5901) => "OpenCompanion boot0",
        _ => "boot0 NOT VALID (the DSP cannot boot)",
    };
    let header = match ImageHeader::parse(&u.ee_read(HDR, EE_PAGE)?) {
        Ok(h) => format!("image seq {}, fw 0x{:08x}, body {} bytes", h.seq, h.fw_version, h.body_len),
        Err(e) => format!("image header: {e}"),
    };
    Ok(format!("{boot0}, {header}"))
}

pub fn dsp(link: &dyn Link) -> DspStatus {
    let runtime = detect(link);
    let eeprom = match runtime {
        DspRuntime::Updater { .. } => Some(updater_eeprom(&Updater::new(link)).unwrap_or_else(|e| e.to_string())),
        _ => None,
    };
    DspStatus { runtime, eeprom }
}
