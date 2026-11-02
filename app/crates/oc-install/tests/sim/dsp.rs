//! Boot decisions use `oc_ais::eeprom::verify`, the host-side model of boot1.

use oc_ais::eeprom::layout::{BOOT0, EE_PAGE, HDR, RAM_TRAMP, STOCK_IMAGE_END};
use oc_ais::eeprom::verify::{verify, Expected, Outcome};
use oc_install::dsp::DspBundle;
use oc_ramload::{stock_ram, HIJACK_FLAGS, HIJACK_ORIGINAL, HIJACK_SLOT, LOADER_HI, LOADER_LO, RAM_HI, RAM_LO};

use super::Speaker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspRun {
    Stock,
    Stub,
    Updater,
    Rescue(u8),
    Oc,
    Dead,
}

pub struct Dsp {
    pub ee: Vec<u8>,
    pub ram: Vec<u8>,
    pub run: DspRun,
    pub unlocked: bool,
    /// `RSC` needs RAM to survive the RTI reset.
    pub ram_retained: bool,
    rescue_flag: bool,
    /// From the next boot on, `SVM` gets a garbled reply (a stale relay frame, as TAS 0.4.9 with the stock DSP).
    pub garble_after_boot: bool,
    garbled: bool,
    /// The stock firmware's `SVM` answer (a Bose soft update changes it).
    pub svm: &'static [u8],
    /// The stock firmware answers the raw EEPROM read `d` (`[addr H][addr L][n]` -> n bytes).
    pub raw_read: bool,
}

const ACK: u8 = 0x06;
const NAK: u8 = 0x15;

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn le16(b: &[u8], o: usize) -> usize {
    u16::from_le_bytes([b[o], b[o + 1]]) as usize
}

impl Dsp {
    pub fn new(stock: Vec<u8>) -> Self {
        Self { ee: stock, ram: vec![0; (RAM_HI - RAM_LO) as usize], run: DspRun::Dead, unlocked: false, ram_retained: true, rescue_flag: false, garble_after_boot: false, garbled: false, raw_read: false, svm: b"D01.01.01U01.01.03\r>" }
    }

    fn ram_range(&self, addr: u32, n: usize) -> Option<std::ops::Range<usize>> {
        let off = addr.checked_sub(RAM_LO)? as usize;
        (off + n <= self.ram.len()).then_some(off..off + n)
    }

    pub fn boot(&mut self, b: &DspBundle) {
        self.unlocked = false;
        self.garbled = self.garble_after_boot;
        let rescue = std::mem::take(&mut self.rescue_flag);
        if !(self.ram_retained && rescue) {
            self.ram.fill(0);
        }
        if self.ee[BOOT0] == b.stock[BOOT0] {
            if self.ee[..STOCK_IMAGE_END] == b.stock[..STOCK_IMAGE_END] {
                self.run = DspRun::Stock;
                let stock = stock_ram(&b.stock).unwrap();
                self.ram.copy_from_slice(&stock);
                let o = (HIJACK_FLAGS - RAM_LO) as usize;
                self.ram[o..o + 4].copy_from_slice(&3u32.to_le_bytes());
                let o = (HIJACK_SLOT - RAM_LO) as usize;
                // The process pointer of the revision the stock firmware answers (`SVM`).
                let svm = String::from_utf8_lossy(self.svm);
                let process = oc_install::dsp::revision::for_svm(&svm).map_or(HIJACK_ORIGINAL, |r| r.process);
                self.ram[o..o + 4].copy_from_slice(&process.to_le_bytes());
            } else {
                self.run = DspRun::Dead;
            }
            return;
        }
        let exp = Expected { boot0: Some(&b.boot0), boot1: Some(&b.boot1) };
        self.run = match verify(&self.ee, &exp).outcome {
            // The real trampoline (232 bytes) spans both pages: any other boot0 does not boot.
            _ if self.ee[BOOT0] != b.boot0 => DspRun::Dead,
            Outcome::NoBootChain { .. } => DspRun::Dead,
            _ if rescue && self.ram_retained => DspRun::Rescue(1),
            Outcome::Boots { .. } => DspRun::Oc,
            Outcome::Rescue(_) => DspRun::Rescue(3),
        };
    }

    fn loaded(&self, img: &oc_ramload::LoadImage) -> bool {
        img.segments.iter().all(|(a, d)| self.ram_range(*a, d.len()).is_some_and(|r| self.ram[r] == d[..]))
            && img.fills.iter().all(|(a, n, v)| self.ram_range(*a, *n as usize).is_some_and(|r| self.ram[r].iter().all(|x| x == v)))
    }
}

impl Speaker {
    /// `E` payload → DSP reply; `Err` = the TAS relay reports ERROR1 (DSP silent).
    pub fn dsp_e(&mut self, m: &[u8]) -> Result<Vec<u8>, ()> {
        match self.dsp.run {
            DspRun::Dead => Err(()),
            DspRun::Stock => Ok(self.stock_console(m)),
            DspRun::Oc => Ok(self.oc_console(m)),
            _ => Ok(self.loader(m)),
        }
    }

    fn stock_console(&mut self, m: &[u8]) -> Vec<u8> {
        if m.starts_with(b"SVM\r") && self.dsp.garbled {
            return vec![0, 0];
        }
        if m.starts_with(b"SVM\r") {
            return self.dsp.svm.to_vec();
        }
        let d = &mut self.dsp;
        if m.starts_with(b"BRR") && m.len() >= 11 {
            let (n, a) = (le32(m, 3) as usize, le32(m, 7));
            return match d.ram_range(a, 4 * n) {
                Some(r) if (1..=4).contains(&n) => [&[ACK][..], &d.ram[r]].concat(),
                _ => vec![NAK],
            };
        }
        if m.starts_with(b"BWW") && m.len() >= 11 {
            let (n, a) = (le32(m, 3) as usize, le32(m, 7));
            let Some(r) = d.ram_range(a, 4 * n).filter(|_| m.len() >= 11 + 4 * n) else { return vec![NAK] };
            d.ram[r].copy_from_slice(&m[11..11 + 4 * n]);
            let stub = &self.known.dsp.stub;
            if a == HIJACK_SLOT && le32(m, 11) == stub.entry && d.loaded(stub) {
                d.run = DspRun::Stub;
            }
            return vec![ACK];
        }
        b"\r!".to_vec()
    }

    fn oc_console(&mut self, m: &[u8]) -> Vec<u8> {
        let known = self.known.clone();
        if m.starts_with(b"SVM\r") && self.dsp.garbled {
            return vec![0, 0];
        }
        if m.starts_with(b"SVM\r") {
            return b"DOC0.0.1U01.01.03\r>".to_vec();
        }
        if m.starts_with(b"UPD\r") {
            self.dsp.ee[HDR..HDR + EE_PAGE].fill(0);
            self.ee_writes += 1;
            self.dsp.boot(&known.dsp);
            return b"UPD\r>".to_vec();
        }
        if m.starts_with(b"RSC\r") {
            self.dsp.rescue_flag = true;
            self.dsp.boot(&known.dsp);
            return b"RSC\r>".to_vec();
        }
        b"\r!".to_vec()
    }

    fn loader(&mut self, m: &[u8]) -> Vec<u8> {
        let known = self.known.clone();
        let run = self.dsp.run;
        let Some(&sub) = m.first() else { return vec![NAK] };
        let a = if m.len() >= 5 { le32(m, 1) } else { 0 };
        let write_ok = |a: u32, n: usize| match run {
            DspRun::Stub => a >= RAM_LO && (a as u64 + n as u64) <= RAM_HI as u64 && (a + n as u32 <= LOADER_LO || a >= LOADER_HI),
            _ => a >= RAM_LO && (a as u64 + n as u64) <= RAM_TRAMP as u64,
        };
        let d = &mut self.dsp;
        match sub {
            0x00 => match run {
                DspRun::Stub => b"\x06OCL\x01".to_vec(),
                DspRun::Rescue(r) => vec![ACK, b'O', b'C', b'L', 1, b'R', r, 1],
                _ => vec![ACK, b'O', b'C', b'L', 1, b'U', 9, 1],
            },
            0x01 if m.len() >= 5 && write_ok(a, m.len() - 5) => {
                let r = d.ram_range(a, m.len() - 5).unwrap();
                d.ram[r].copy_from_slice(&m[5..]);
                vec![ACK]
            }
            0x02 if m.len() >= 9 => match d.ram_range(a, le32(m, 5) as usize) {
                Some(r) => [&[ACK][..], &crc32fast::hash(&d.ram[r]).to_le_bytes()].concat(),
                None => vec![NAK],
            },
            0x03 if m.len() >= 5 => {
                d.run = if a == known.dsp.updater.entry && d.loaded(&known.dsp.updater) { DspRun::Updater } else { DspRun::Dead };
                d.unlocked = false;
                vec![ACK]
            }
            0x05 if m.len() >= 10 && write_ok(a, le32(m, 5) as usize) => {
                let r = d.ram_range(a, le32(m, 5) as usize).unwrap();
                d.ram[r].fill(m[9]);
                vec![ACK]
            }
            0x06 => {
                d.boot(&known.dsp);
                vec![ACK]
            }
            0x10..=0x14 if run == DspRun::Updater => self.updater_ee(m),
            _ => vec![NAK],
        }
    }

    fn ee_write(&mut self, addr: usize, data: &[u8]) {
        let data = self.maybe_corrupt(data);
        self.dsp.ee[addr..addr + data.len()].copy_from_slice(&data);
        self.ee_writes += 1;
    }

    fn updater_ee(&mut self, m: &[u8]) -> Vec<u8> {
        let nak = |c: u8| vec![NAK, c];
        match m[0] {
            0x10 if m.len() >= 4 => {
                let (a, n) = (le16(m, 1), m[3] as usize);
                if n == 0 || n > 60 || a + n > self.dsp.ee.len() {
                    return nak(1);
                }
                [&[ACK][..], &self.dsp.ee[a..a + n]].concat()
            }
            0x11 if m.len() >= 4 => {
                let (a, data) = (le16(m, 1), &m[3..]);
                let end = a + data.len();
                let in_window = (a >= 0x0100 && end <= 0xDB00) || (self.dsp.unlocked && end <= 0xFF80);
                if data.len() > 56 || a / EE_PAGE != (end - 1) / EE_PAGE || !in_window {
                    return nak(1);
                }
                self.ee_write(a, data);
                if self.dsp.ee[a..end] != *data {
                    return nak(3);
                }
                vec![ACK]
            }
            0x12 if m.len() >= 7 => {
                let (a, n) = (le16(m, 1), le32(m, 3) as usize);
                assert!(n <= 256, "EE_CRC over {n} bytes: longer than the 20 ms TAS reply delay allows");
                if a + n > self.dsp.ee.len() {
                    return nak(1);
                }
                [&[ACK][..], &crc32fast::hash(&self.dsp.ee[a..a + n]).to_le_bytes()].concat()
            }
            0x13 if m.len() >= 5 && &m[1..5] == b"UNLK" => {
                self.dsp.unlocked = true;
                vec![ACK]
            }
            0x14 if m.len() >= 9 => {
                let (ram, crc) = (le32(m, 1), le32(m, 5));
                self.boot0_ops.push((self.ops - 1, self.awake));
                let Some(r) = self.dsp.ram_range(ram, 256).filter(|_| self.dsp.unlocked) else { return nak(1) };
                let src = self.dsp.ram[r].to_vec();
                if crc32fast::hash(&src) != crc {
                    return nak(4);
                }
                if self.mid {
                    // Power lost between the two page writes: page 0 new, page 1 stock -> brick.
                    self.dsp.ee[..EE_PAGE].copy_from_slice(&src[..EE_PAGE]);
                    self.ee_writes += 1;
                    return vec![ACK];
                }
                self.dsp.ee[..256].copy_from_slice(&src);
                self.ee_writes += 2;
                vec![ACK]
            }
            _ => nak(1),
        }
    }
}
