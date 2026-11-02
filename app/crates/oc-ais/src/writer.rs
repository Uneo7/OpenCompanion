use crate::format::*;
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub ram: (u32, u32),
    /// Maximum image size (U7001 EEPROM: 64 KiB).
    pub max_size: usize,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self { ram: (0x1000_0000, 0x1003_0000), max_size: 64 * 1024 }
    }
}

/// Builds an image: disable CRC, load segments, zero fills, final jump.
pub fn build(segments: &[Segment], zero_fills: &[(u32, u32)], entry: u32, opts: &BuildOptions) -> Result<Image> {
    let in_ram = |addr: u32, len: usize| addr >= opts.ram.0 && (addr as u64 + len as u64) <= opts.ram.1 as u64;
    let mut commands = vec![Command::DisableCrc];
    for s in segments {
        if !in_ram(s.addr, s.data.len()) {
            return Err(Error::OutOfRange { addr: s.addr, len: s.data.len() });
        }
        commands.push(Command::SectionLoad(s.clone()));
    }
    for &(addr, size) in zero_fills {
        if !in_ram(addr, size as usize) {
            return Err(Error::OutOfRange { addr, len: size as usize });
        }
        commands.push(Command::SectionFill { addr, size, kind: 0, pattern: 0 });
    }
    commands.push(Command::JumpClose(entry));
    Ok(Image { commands })
}

/// Serializes an image to bytes. Bose zlib sections are re-emitted as plain section loads.
pub fn write(image: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut w = |v: u32| out.extend_from_slice(&v.to_le_bytes());
    w(MAGIC);
    for c in &image.commands {
        match c {
            Command::SectionLoad(s) | Command::BoseZlib(s) => {
                w(OP_SECTION_LOAD);
                w(s.addr);
                w(s.data.len() as u32);
                out_data(&mut w, &s.data);
            }
            Command::SectionFill { addr, size, kind, pattern } => {
                for v in [OP_SECTION_FILL, *addr, *size, *kind, *pattern] {
                    w(v);
                }
            }
            Command::Set { kind, addr, data, sleep } => {
                for v in [OP_SET, *kind, *addr, *data, *sleep] {
                    w(v);
                }
            }
            Command::Jump(a) => {
                w(OP_JUMP);
                w(*a);
            }
            Command::JumpClose(a) => {
                w(OP_JUMP_CLOSE);
                w(*a);
            }
            Command::EnableCrc => w(OP_ENABLE_CRC),
            Command::DisableCrc => w(OP_DISABLE_CRC),
            Command::RequestCrc { crc, seek } => {
                w(OP_REQUEST_CRC);
                w(*crc);
                w(*seek as u32);
            }
        }
    }
    Ok(out)
}

fn out_data(w: &mut impl FnMut(u32), data: &[u8]) {
    for chunk in data.chunks(4) {
        let mut b = [0u8; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        w(u32::from_le_bytes(b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn round_trip() {
        let segs = vec![Segment { addr: 0x1000_0000, data: vec![1, 2, 3, 4, 5] }];
        let img = build(&segs, &[(0x1000_1000, 16)], 0x1000_0000, &BuildOptions::default()).unwrap();
        let bytes = write(&img).unwrap();
        let back = parse(&bytes).unwrap();
        assert_eq!(back.entry(), Some(0x1000_0000));
        let flat = back.flatten(0x1000_0000);
        assert_eq!(&flat[..5], &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn rejects_out_of_ram() {
        let segs = vec![Segment { addr: 0x0000_1000, data: vec![0; 4] }];
        assert!(build(&segs, &[], 0, &BuildOptions::default()).is_err());
    }
}
