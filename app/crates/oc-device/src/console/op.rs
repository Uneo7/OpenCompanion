//! Binary `OP*` commands: little-endian arguments, no `\r`; replies `06` + data or `15 <code>`.

use std::ops::Range;

use super::options::DeviceOptions;
use super::Dsp;
use crate::error::{Error, Result};
use crate::params::{name_from_bytes, write_ranges, Profile, NAME_LEN, PROFILE_SIZE};

pub const OPR_MAX: usize = 56;
/// Firmware-side limit; the link may allow less ([`Dsp::opw_max`]).
pub const OPW_MAX: usize = 55;
pub const SLOTS: usize = 4;
pub const SLOT_NONE: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpInfo {
    /// 2 = with `OPQ`, `OPS` and `bass_milli`; 3 = with `OPU` (routage 2.1) and [`OpInfo::graph_missing`];
    /// 4 = with `OPC` (device options, [`Route21::options`]) and the band enabled bit ([`crate::params::BAND_DISABLED`]);
    /// 5 = unified engine: `OPQ` / `OPS` are the sub level source ([`BassState`] in 1/100 dB), `crossover_hz` and
    /// `bass_milli` reserved, 2.1 routing from the TAS mode ([`Route21::active`]).
    pub proto: u8,
    pub profile_size: u16,
    pub slots: u8,
    pub active: Option<u8>,
    pub modified: bool,
    pub busy: bool,
    pub last_failed: bool,
    /// The stock graph tables are missing from the EEPROM (graph data pages): a profile on the Bose base plays
    /// through the EQ path instead (flags bit 3, proto >= 3).
    pub graph_missing: bool,
    pub boot: Option<u8>,
    pub valid_mask: u8,
    pub writable_mask: u8,
    pub last_result: u8,
    pub sections: u16,
    /// %, measured by the firmware (proto >= 3).
    pub cpu_pct: u8,
    pub profile_flags: u8,
}

fn slot(b: u8) -> Option<u8> {
    (b != SLOT_NONE).then_some(b)
}

impl OpInfo {
    /// `data` = reply after the ACK byte.
    pub fn parse(d: &[u8]) -> Option<Self> {
        if d.len() < 14 {
            return None;
        }
        Some(Self {
            proto: d[0],
            profile_size: u16::from_le_bytes([d[1], d[2]]),
            slots: d[3],
            active: slot(d[4]),
            modified: d[5] & 1 != 0,
            busy: d[5] & 2 != 0,
            last_failed: d[5] & 4 != 0,
            graph_missing: d[5] & 8 != 0,
            boot: slot(d[6]),
            valid_mask: d[7],
            writable_mask: d[8],
            last_result: d[9],
            sections: u16::from_le_bytes([d[10], d[11]]),
            cpu_pct: d[12],
            profile_flags: d[13],
        })
    }

    pub fn slot_valid(&self, s: usize) -> bool {
        self.valid_mask & (1 << s) != 0
    }

    pub fn slot_writable(&self, s: usize) -> bool {
        self.writable_mask & (1 << s) != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SlotEntry {
    pub valid: bool,
    pub color: u8,
    /// Higher = newer.
    pub seq: u32,
    pub name: String,
}

impl SlotEntry {
    pub fn parse(d: &[u8]) -> Option<Self> {
        if d.len() < 6 + NAME_LEN {
            return None;
        }
        Some(Self {
            valid: d[0] != 0,
            color: d[1],
            seq: u32::from_le_bytes([d[2], d[3], d[4], d[5]]),
            name: name_from_bytes(&d[6..6 + NAME_LEN]),
        })
    }
}

/// Config record byte 9 (`OPS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BassSource {
    /// The rear bass knob sets the sub level (stock behaviour, default): centre detent 0 dB, −12 dB fully left,
    /// +6 dB fully right ([`BassState::knob`]).
    #[default]
    Knob,
    /// The active profile's `sub_level_cdb` ("app"); the knob is ignored but still reported.
    Software,
}

/// From this protocol `OPQ` is in 1/100 dB.
pub const PROTO_SUB_LEVEL: u8 = 5;

/// Structural change between two profiles, as the DSP classifies it (`params_write`): flags or any band type
/// (incl. its enabled bit). Gain / frequency / Q edits glide and need no commit.
pub fn structural(a: &Profile, b: &Profile) -> bool {
    a.flags != b.flags || a.bands.iter().flatten().zip(b.bands.iter().flatten()).any(|(x, y)| x.kind != y.kind)
}

/// First DSP profile protocol where the "app" sub level is a device setting (config record, `DEVL`), not the
/// profile's `sub_level_cdb` (now ignored).
pub const PROTO_SUB_DEVICE: u8 = 8;

pub const PROTO_OPTIONS: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Route21 {
    /// Setting (saved in the config record). Protocol 5: always `false` (`OPU` removed, the TAS mode decides).
    pub on: bool,
    /// In effect now. Protocol 5: TAS 2.1 mode (`R` layout bit1) on the neutral preset (the Bose preset keeps
    /// the stock routing).
    pub active: bool,
    /// Input layout of the `R` frame: 1 = 5.1, 2 = 2.1 (protocol 5: 2.1 mode, 24-bit stereo; 3 no longer used).
    pub layout: u8,
    /// Config record byte 11; `None` before DSP protocol 4.
    pub options: Option<DeviceOptions>,
}

impl Route21 {
    pub fn parse(d: &[u8]) -> Option<Self> {
        (d.len() >= 3).then(|| Self {
            on: d[0] != 0,
            active: d[1] != 0,
            layout: d[2],
            options: d.get(3).copied().map(DeviceOptions::from_byte),
        })
    }
}

/// `OPQ` reply. Protocol 5 (sub level): 1/100 dB. Before: ×1000 on the bass knob scale (−1667..+1667).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BassState {
    pub source: BassSource,
    /// Sub level of the "app" source, 1/100 dB: a device setting from [`PROTO_SUB_DEVICE`] (before: the active
    /// profile's `sub_level_cdb`).
    pub soft: i16,
    /// The rear knob as a sub level, exactly the gain the firmware applies in knob mode (−1200..+600, centre
    /// detent 0; always read).
    pub knob: i16,
    /// Before the sub trim.
    pub value: i16,
}

impl BassState {
    /// The rear knob as a sub level in dB, for the DSP profile protocol `proto` ([`OpInfo::proto`]): from
    /// [`PROTO_SUB_LEVEL`] the exact gain the firmware applies in knob mode (1/100 dB); before, the knob
    /// position (×1000 of the stock scale, ±1667) shown on a ±12 dB scale.
    pub fn knob_db(&self, proto: u8) -> f32 {
        if proto >= PROTO_SUB_LEVEL {
            f32::from(self.knob) / 100.0
        } else {
            f32::from(self.knob) / 1667.0 * 12.0
        }
    }
}

impl BassState {
    pub fn parse(d: &[u8]) -> Option<Self> {
        if d.len() < 7 {
            return None;
        }
        let at = |o: usize| i16::from_le_bytes([d[o], d[o + 1]]);
        let source = if d[0] == 1 { BassSource::Software } else { BassSource::Knob };
        Some(Self { source, soft: at(1), knob: at(3), value: at(5) })
    }
}

impl Dsp<'_> {
    /// `OPW` + offset take 5 bytes.
    pub fn opw_max(&self) -> usize {
        OPW_MAX.min(self.link.max_payload().saturating_sub(5))
    }

    pub fn opr_max(&self) -> usize {
        OPR_MAX.min(self.link.max_reply().saturating_sub(1))
    }

    fn op(&self, cmd: &'static str, args: &[u8]) -> Result<Vec<u8>> {
        let mut req = cmd.as_bytes().to_vec();
        req.extend_from_slice(args);
        let reply = self.console().raw(&req)?;
        match reply.split_first() {
            Some((0x06, rest)) => Ok(rest.to_vec()),
            Some((0x15, rest)) => Err(Error::Nak { cmd, code: rest.first().copied().unwrap_or(0) }),
            _ => Err(Error::Reply { cmd, reply: format!("{reply:02x?}") }),
        }
    }

    fn parsed<T>(&self, cmd: &'static str, args: &[u8], parse: impl Fn(&[u8]) -> Option<T>) -> Result<T> {
        let d = self.op(cmd, args)?;
        parse(&d).ok_or_else(|| Error::Reply { cmd, reply: format!("{d:02x?}") })
    }

    pub fn op_info(&self) -> Result<OpInfo> {
        self.parsed("OPI", &[], OpInfo::parse)
    }

    pub fn op_read(&self, off: usize, n: usize) -> Result<Vec<u8>> {
        let d = self.op("OPR", &[off as u8, (off >> 8) as u8, n as u8])?;
        if d.len() < n {
            return Err(Error::Reply { cmd: "OPR", reply: format!("{} bytes instead of {n}", d.len()) });
        }
        Ok(d[..n].to_vec())
    }

    /// Applied at once, sanitized by the DSP.
    pub fn op_write(&self, off: usize, data: &[u8]) -> Result<()> {
        let mut args = vec![off as u8, (off >> 8) as u8];
        args.extend_from_slice(data);
        self.op("OPW", &args).map(drop)
    }

    pub fn read_profile(&self) -> Result<Profile> {
        let step = self.opr_max();
        let mut bytes = Vec::with_capacity(PROFILE_SIZE);
        while bytes.len() < PROFILE_SIZE {
            let n = step.min(PROFILE_SIZE - bytes.len());
            bytes.extend(self.op_read(bytes.len(), n)?);
        }
        Ok(Profile::from_bytes(&bytes)?)
    }

    pub fn write_profile(&self, from: &Profile, to: &Profile) -> Result<Vec<Range<usize>>> {
        let bytes = to.to_bytes();
        let ranges = write_ranges(from, to, self.opw_max());
        for r in &ranges {
            self.op_write(r.start, &bytes[r.clone()])?;
        }
        if structural(from, to) {
            // DSP protocol 9: the batch is complete, install it now behind one output ramp (older firmware: unknown
            // command, ignored; the DSP then closes the batch 50 ms after the last write).
            let _ = self.op_commit();
        }
        Ok(ranges)
    }

    /// `OPX` (DSP protocol 9): a batch of `OPW` that changed the structure is complete.
    pub fn op_commit(&self) -> Result<()> {
        self.op("OPX", &[]).map(drop)
    }

    pub fn op_defaults(&self) -> Result<()> {
        self.op("OPD", &[]).map(drop)
    }

    /// Asynchronous: poll [`Dsp::op_info`] until not busy.
    pub fn op_load(&self, s: u8) -> Result<()> {
        self.op("OPL", &[s]).map(drop)
    }

    /// Asynchronous (~50 ms).
    pub fn op_save(&self, s: u8) -> Result<()> {
        self.op("OPV", &[s]).map(drop)
    }

    /// `None` = flat defaults.
    pub fn op_boot(&self, s: Option<u8>) -> Result<()> {
        self.op("OPB", &[s.unwrap_or(SLOT_NONE)]).map(drop)
    }

    pub fn op_name(&self, s: u8) -> Result<SlotEntry> {
        self.parsed("OPN", &[s], SlotEntry::parse)
    }

    pub fn op_bose(&self) -> Result<()> {
        self.op("OPK", &[]).map(drop)
    }

    pub fn op_flat(&self) -> Result<()> {
        self.op("OPD", &[]).map(drop)
    }

    pub fn op_route21(&self) -> Result<Route21> {
        self.parsed("OPU", &[], Route21::parse)
    }

    /// `OPC` without argument (DSP protocol >= 4; from 5 `OPU` is gone and [`Route21::on`] reads `false`).
    pub fn op_options(&self) -> Result<Route21> {
        self.parsed("OPC", &[], Route21::parse)
    }

    /// "Routage 2.1" (device setting, saved in the config record, asynchronous). DSP protocol 3..4 only: from
    /// protocol 5 the TAS `R` frame selects the 2.1 mode and the firmware refuses `OPU`.
    pub fn op_set_route21(&self, on: bool) -> Result<Route21> {
        self.parsed("OPU", &[u8::from(on)], Route21::parse)
    }

    /// Asynchronous, DSP protocol >= 4; same reply as `OPU`.
    pub fn op_set_options(&self, o: DeviceOptions) -> Result<Route21> {
        self.parsed("OPC", &[o.to_byte()], Route21::parse)
    }

    pub fn op_bass(&self) -> Result<BassState> {
        self.parsed("OPQ", &[], BassState::parse)
    }

    /// Asynchronous.
    pub fn op_bass_source(&self, src: BassSource) -> Result<()> {
        self.op("OPS", &[u8::from(src == BassSource::Software)]).map(drop)
    }

    /// Sub level of the "app" source, dB × 10 (−120..+120): a device setting since DSP protocol 8
    /// ([`PROTO_SUB_DEVICE`]), text command `DEVL<v>`; applied at once (smoothed), saved by the DSP 1 s after the
    /// last change, so a slider may send every step. Read back with [`Dsp::op_bass`] ([`BassState::soft`], cdb).
    /// The "app" sub level as `DEV` reports it (`SL:`, dB × 10; [`PROTO_SUB_DEVICE`]).
    pub fn dev_sub_level(&self) -> Result<i16> {
        let s = self.text("DEV")?;
        parse_sl(&s).ok_or(crate::error::Error::Reply { cmd: "DEV", reply: s })
    }

    /// [`Dsp::op_bass`] with the "app" level read back from `DEV` (the device setting, [`PROTO_SUB_DEVICE`]).
    pub fn bass_device(&self) -> Result<BassState> {
        let mut b = self.op_bass()?;
        b.soft = self.dev_sub_level()?.saturating_mul(10);
        Ok(b)
    }

    pub fn set_sub_level(&self, db10: i16) -> Result<()> {
        let v = db10.clamp(-120, 120);
        let s = self.text(&format!("DEVL{v}"))?;
        if s.contains("R:") { Ok(()) } else { Err(crate::error::Error::Reply { cmd: "DEVL", reply: s }) }
    }
}

/// `SL:<dB × 10>` in a `DEV` reply.
fn parse_sl(s: &str) -> Option<i16> {
    let v = s.split_whitespace().find_map(|w| w.strip_prefix("SL:"))?;
    v.trim_end_matches(['\r', '>']).parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_device_sub_level_is_read_from_dev() {
        assert_eq!(parse_sl("SR:1 SL:-35 KN:0 AC:0 LY:1 OP:3\r>"), Some(-35));
        assert_eq!(parse_sl("SR:1 SL:120 KN:0 AC:0 LY:1 OP:3 R:0\r>"), Some(120));
        assert_eq!(parse_sl("SR:1 KN:0\r>"), None);
    }

    #[test]
    fn info_parses_the_firmware_layout() {
        let d = [2, 0x68, 0x02, 4, 1, 3, 0xFF, 3, 0xF, 0, 21, 0, 4, 4];
        let i = OpInfo::parse(&d).unwrap();
        assert_eq!((i.proto, i.profile_size, i.slots, i.active, i.boot), (2, 616, 4, Some(1), None));
        assert!(i.modified && i.busy && !i.last_failed && !i.graph_missing);
        let mut d3 = d;
        d3[0] = 3;
        d3[5] = 8;
        assert!(OpInfo::parse(&d3).unwrap().graph_missing);
        assert!(i.slot_valid(1) && !i.slot_valid(2) && i.slot_writable(3));
        assert_eq!((i.sections, i.cpu_pct, i.profile_flags), (21, 4, 4));
        assert!(OpInfo::parse(&d[..13]).is_none());
    }

    #[test]
    fn slot_entry_and_bass_parse() {
        let mut d = vec![1, 3, 7, 0, 0, 0];
        d.extend_from_slice(b"Musique\0\0\0\0\0\0\0\0\0");
        let e = SlotEntry::parse(&d).unwrap();
        assert_eq!(e, SlotEntry { valid: true, color: 3, seq: 7, name: "Musique".into() });
        let a = crate::params::Profile::defaults();
        let mut b = a.clone();
        b.bands[1][0].gain_cdb = 300;
        assert!(!structural(&a, &b), "a gain edit glides");
        b.bands[1][0].kind = 1;
        assert!(structural(&a, &b), "a band switched on is structural");
        assert_eq!(Route21::parse(&[1, 0, 3]), Some(Route21 { on: true, active: false, layout: 3, options: None }));
        let r = Route21::parse(&[0, 0, 1, 0x18]).unwrap();
        let o = r.options.unwrap();
        assert!(!o.double_tap && o.headphone_eq && o.standby == super::super::options::StandbyDelay::Stock);
        assert_eq!(Route21::parse(&[1, 0]), None);
        let b = BassState::parse(&[1, 0x0C, 0xFE, 0x10, 0x00, 0x0C, 0xFE]).unwrap();
        assert_eq!(b, BassState { source: BassSource::Software, soft: -500, knob: 16, value: -500 });
        let k = BassState { knob: -1200, ..b };
        assert_eq!(k.knob_db(PROTO_SUB_LEVEL), -12.0);
        assert!((BassState { knob: 1667, ..b }.knob_db(PROTO_OPTIONS) - 12.0).abs() < 1e-6);
    }
}
