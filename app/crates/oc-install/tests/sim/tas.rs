use oc_protocol::oc_hid::status;
use oc_tas_image::checksum;

use super::{Known, Speaker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TasRun {
    Bose,
    Oc(usize),
    /// ROM DFU (DataType 2/3 or invalid header): no vendor HID.
    Dfu,
    /// DataType 1 with unknown code.
    Dead,
}

pub struct Tas {
    pub ee: Vec<u8>,
    pub run: TasRun,
    pub session: bool,
}

impl Tas {
    pub fn new(ee: Vec<u8>) -> Self {
        Self { ee, run: TasRun::Dead, session: false }
    }

    pub fn boot(&mut self, k: &Known) {
        self.session = false;
        let h = &self.ee[..18];
        let shape = h[1] == 0x12 && h[2] == 0x12 && h[3] == 0x34;
        self.run = if !shape || h[14] != 1 || checksum(h) != h[0] {
            TasRun::Dfu
        } else if self.ee[..0x1800] == k.tas_bose[..0x1800] {
            TasRun::Bose // the stock firmware never reads the data area (0x1800..)
        } else if let Some(i) = k.tas_oc.iter().position(|t| {
            // Our firmware stays off USB (no vendor HID) without a valid data area, when its build has one.
            self.ee[..18 + t.image.code.len()] == t.bytes[..18 + t.image.code.len()]
                && (t.data.is_none() || oc_tas_image::data_area::DataArea::parse(&self.ee[0x1800..]).is_ok())
        }) {
            TasRun::Oc(i)
        } else {
            TasRun::Dead
        };
    }
}

impl Speaker {
    fn tas_write(&mut self, addr: usize, data: &[u8]) {
        assert!(!data.is_empty() && addr / 32 == (addr + data.len() - 1) / 32, "TAS write 0x{addr:04x}+{} crosses a 32-byte page", data.len());
        if addr < 18 {
            self.tas_header_ops.push(self.ops - 1);
        }
        let data = self.maybe_corrupt(data);
        self.tas.ee[addr..addr + data.len()].copy_from_slice(&data);
        self.ee_writes += 1;
    }

    pub fn bose_eeprom(&mut self, cmd: u8, d: &[u8]) -> (u8, Vec<u8>) {
        let a = ((d[0] as usize) << 8) | d[1] as usize;
        let n = d[2] as usize;
        if cmd == b't' {
            return (b't', [&d[..3], &self.tas.ee[a..a + n]].concat());
        }
        self.tas_write(a, &d[3..3 + n]);
        (b'T', d[..3].to_vec())
    }

    /// OpenCompanion EE_* commands, same rules as `firmware/usb/app/proto/eeprom.c`.
    pub fn oc_eeprom(&mut self, cmd: u8, p: &[u8]) -> (u8, Vec<u8>) {
        use oc_protocol::oc_hid::{cmd as c, key};
        let addr = if p.len() >= 2 { ((p[0] as usize) << 8) | p[1] as usize } else { 0 };
        match cmd {
            c::EE_READ => {
                let n = p[2] as usize;
                if p.len() != 3 || n == 0 || n > 60 || addr + n > 8192 {
                    return (status::BAD_ARG, vec![]);
                }
                (status::OK, self.tas.ee[addr..addr + n].to_vec())
            }
            c::EE_BEGIN => {
                if p != key(c::EE_BEGIN) {
                    return (status::BAD_KEY, vec![]);
                }
                let h = &self.tas.ee;
                if !(h[1] == 0x12 && h[2] == 0x12 && h[3] == 0x34) {
                    return (status::EE_HEADER, vec![]);
                }
                self.tas_write(14, &[3]);
                self.tas.session = true;
                (status::OK, vec![])
            }
            c::EE_WRITE => {
                let data = &p[2..];
                let n = data.len();
                if !self.tas.session {
                    return (status::STATE, vec![]);
                }
                if n == 0 || n > 32 || addr < 0x12 || addr + n > 0x2000 || (addr + n > 0x12 + 6016 && addr < 0x1800)
                    || (addr & 31) + n > 32
                {
                    return (status::EE_RANGE, vec![]);
                }
                self.tas_write(addr, data);
                (if self.tas.ee[addr..addr + n] == *data { status::OK } else { status::EE_VERIFY }, vec![])
            }
            c::EE_COMMIT => {
                if !self.tas.session {
                    return (status::STATE, vec![]);
                }
                if p.len() != 18 || !(p[1] == 0x12 && p[2] == 0x12 && p[3] == 0x34) || checksum(p) != p[0] || p[14] != 1 {
                    return (status::EE_HEADER, vec![]);
                }
                self.tas_write(0, &p[..14]);
                self.tas_write(15, &p[15..18]);
                self.tas_write(14, &p[14..15]);
                self.tas.session = false;
                (status::OK, vec![])
            }
            _ => {
                self.tas.session = false;
                (status::OK, vec![])
            }
        }
    }
}
