use std::io::Read;

use crate::format::*;
use crate::{Error, Result};

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn u32(&mut self) -> Result<u32> {
        let b = self.data.get(self.pos..self.pos + 4).ok_or(Error::Truncated(self.pos))?;
        self.pos += 4;
        Ok(u32::from_le_bytes(b.try_into().unwrap()))
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let b = self.data.get(self.pos..self.pos + n).ok_or(Error::Truncated(self.pos))?;
        self.pos = (self.pos + n + 3) & !3; // data is padded to 4 bytes
        Ok(b)
    }
}

fn inflate(raw: &[u8], at: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(raw).read_to_end(&mut out).map_err(|_| Error::BadZlib(at))?;
    Ok(out)
}

pub fn parse(data: &[u8]) -> Result<Image> {
    let mut c = Cursor { data, pos: 0 };
    let magic = c.u32()?;
    if magic != MAGIC {
        return Err(Error::BadMagic(magic));
    }
    let mut commands = Vec::new();
    // Bose zlib section in progress: (address, accumulated compressed data).
    let mut pending: Option<(u32, Vec<u8>)> = None;

    while c.pos + 4 <= data.len() {
        let at = c.pos;
        let op = c.u32()?;
        let cmd = match op {
            OP_SECTION_LOAD => {
                let addr = c.u32()?;
                let n = c.u32()? as usize;
                Command::SectionLoad(Segment { addr, data: c.bytes(n)?.to_vec() })
            }
            OP_SECTION_FILL => Command::SectionFill { addr: c.u32()?, size: c.u32()?, kind: c.u32()?, pattern: c.u32()? },
            OP_SET => Command::Set { kind: c.u32()?, addr: c.u32()?, data: c.u32()?, sleep: c.u32()? },
            OP_JUMP => Command::Jump(c.u32()?),
            OP_ENABLE_CRC => Command::EnableCrc,
            OP_DISABLE_CRC => Command::DisableCrc,
            OP_REQUEST_CRC => Command::RequestCrc { crc: c.u32()?, seek: c.u32()? as i32 },
            OP_JUMP_CLOSE => {
                commands.push(Command::JumpClose(c.u32()?));
                break;
            }
            OP_BOSE_ZLIB => {
                // Flags: 1 = single block (address, size), 3 = first chunk (address, size),
                // 2 = next chunk (size), 0 = last chunk (size).
                match c.u32()? {
                    1 => {
                        let addr = c.u32()?;
                        let n = c.u32()? as usize;
                        let raw = c.bytes(n)?;
                        commands.push(Command::BoseZlib(Segment { addr, data: inflate(raw, at)? }));
                    }
                    3 => {
                        let addr = c.u32()?;
                        let n = c.u32()? as usize;
                        pending = Some((addr, c.bytes(n)?.to_vec()));
                    }
                    flag @ (2 | 0) => {
                        let n = c.u32()? as usize;
                        let raw = c.bytes(n)?;
                        let (addr, mut acc) = pending.take().ok_or(Error::BadZlib(at))?;
                        acc.extend_from_slice(raw);
                        if flag == 0 {
                            commands.push(Command::BoseZlib(Segment { addr, data: inflate(&acc, at)? }));
                        } else {
                            pending = Some((addr, acc));
                        }
                    }
                    _ => return Err(Error::BadZlib(at)),
                }
                continue;
            }
            opcode => return Err(Error::UnknownOpcode { opcode, offset: at }),
        };
        commands.push(cmd);
    }
    Ok(Image { commands })
}
