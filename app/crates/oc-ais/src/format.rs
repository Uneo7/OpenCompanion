/// Magic word "TIPA" (read little-endian).
pub const MAGIC: u32 = 0x4150_4954;

pub(crate) const OP_SECTION_LOAD: u32 = 0x5853_5901;
pub(crate) const OP_REQUEST_CRC: u32 = 0x5853_5902;
pub(crate) const OP_ENABLE_CRC: u32 = 0x5853_5903;
pub(crate) const OP_DISABLE_CRC: u32 = 0x5853_5904;
pub(crate) const OP_JUMP: u32 = 0x5853_5905;
pub(crate) const OP_JUMP_CLOSE: u32 = 0x5853_5906;
pub(crate) const OP_SET: u32 = 0x5853_5907;
pub(crate) const OP_BOSE_ZLIB: u32 = 0x5853_5909;
pub(crate) const OP_SECTION_FILL: u32 = 0x5853_590A;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub addr: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    SectionLoad(Segment),
    /// Compressed section (Bose extension, opcode 0x09), already inflated.
    BoseZlib(Segment),
    SectionFill { addr: u32, size: u32, kind: u32, pattern: u32 },
    Set { kind: u32, addr: u32, data: u32, sleep: u32 },
    Jump(u32),
    JumpClose(u32),
    EnableCrc,
    DisableCrc,
    RequestCrc { crc: u32, seek: i32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
    pub commands: Vec<Command>,
}

impl Image {
    /// Entry point (last Jump_Close), if any.
    pub fn entry(&self) -> Option<u32> {
        self.commands.iter().rev().find_map(|c| match c {
            Command::JumpClose(a) => Some(*a),
            _ => None,
        })
    }

    pub fn segments(&self) -> impl Iterator<Item = &Segment> {
        self.commands.iter().filter_map(|c| match c {
            Command::SectionLoad(s) | Command::BoseZlib(s) => Some(s),
            _ => None,
        })
    }

    /// Flat RAM image starting at `base` (fills included).
    pub fn flatten(&self, base: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut put = |addr: u32, bytes: &[u8]| {
            if addr < base {
                return;
            }
            let off = (addr - base) as usize;
            if out.len() < off + bytes.len() {
                out.resize(off + bytes.len(), 0);
            }
            out[off..off + bytes.len()].copy_from_slice(bytes);
        };
        for c in &self.commands {
            match c {
                Command::SectionLoad(s) | Command::BoseZlib(s) => put(s.addr, &s.data),
                Command::SectionFill { addr, size, pattern, .. } => {
                    let fill: Vec<u8> = pattern.to_le_bytes().iter().copied().cycle().take(*size as usize).collect();
                    put(*addr, &fill);
                }
                _ => {}
            }
        }
        out
    }
}
