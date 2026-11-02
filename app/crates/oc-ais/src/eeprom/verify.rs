use super::body;
use super::graph::GraphData;
use super::header::{BootState, ImageHeader, BOOT_TRIALS, HF_TRIAL};
use super::layout::{profile_copy, APP_RAM, BODY, BOOT0, BOOT1, CONFIG_COPIES, EE_PAGE, EE_SIZE, HDR, RAM_TRAMP, STATE, TRAMP_MAX};
use crate::format::{MAGIC, OP_JUMP_CLOSE, OP_SECTION_LOAD};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub level: Level,
    pub area: &'static str,
    pub msg: String,
}

/// What the boot chain does with this content (no rescue request pending).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The ROM cannot run boot0 (not migrated, or corrupted). `stock` = the stock Bose boot is intact.
    NoBootChain { stock: bool },
    Boots { entry: u32, seq: u32 },
    Rescue(String),
}

#[derive(Debug, Clone)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub outcome: Outcome,
}

impl Report {
    pub fn worst(&self) -> Level {
        self.findings.iter().map(|f| f.level).max().unwrap_or(Level::Info)
    }
}

/// Expected boot binaries (from the ELF files); `None` = only structural checks.
#[derive(Default)]
pub struct Expected<'a> {
    pub boot0: Option<&'a [u8]>,
    pub boot1: Option<&'a [u8]>,
}

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

struct Ctx {
    f: Vec<Finding>,
}

impl Ctx {
    fn add(&mut self, level: Level, area: &'static str, msg: impl Into<String>) {
        self.f.push(Finding { level, area, msg: msg.into() });
    }
}

/// boot0 structure: magic, Section Load of <= TRAMP_MAX bytes at RAM_TRAMP, Jump_Close RAM_TRAMP.
fn check_boot0(c: &mut Ctx, b: &[u8], exp: Option<&[u8]>) -> bool {
    let ok = le32(b, 0) == MAGIC && le32(b, 4) == OP_SECTION_LOAD && le32(b, 8) == RAM_TRAMP && {
        let n = le32(b, 12) as usize;
        n <= TRAMP_MAX && n.is_multiple_of(4) && le32(b, 16 + n) == OP_JUMP_CLOSE && le32(b, 20 + n) == RAM_TRAMP
    };
    if !ok {
        return false;
    }
    match exp {
        Some(e) if e != b => c.add(Level::Error, "boot0", "differs from the expected boot0"),
        Some(_) => c.add(Level::Info, "boot0", "matches the expected boot0"),
        None => c.add(Level::Info, "boot0", format!("trampoline of {} bytes at 0x{RAM_TRAMP:08x}", le32(b, 12))),
    }
    true
}

fn check_boot1(c: &mut Ctx, b: &[u8], exp: Option<&[u8]>) -> bool {
    let n = b.len() - 4;
    let crc_ok = crc32fast::hash(&b[..n]) == le32(b, n);
    if !crc_ok {
        c.add(Level::Error, "boot1", "CRC-32 mismatch: boot1 is damaged or missing (the boot chain is broken)");
    }
    match exp {
        Some(e) if e != b => c.add(Level::Error, "boot1", "differs from the expected boot1"),
        Some(_) => c.add(Level::Info, "boot1", "matches the expected boot1"),
        None if crc_ok => c.add(Level::Info, "boot1", "CRC-32 OK"),
        None => {}
    }
    crc_ok && exp.is_none_or(|e| e == b)
}

fn check_image(c: &mut Ctx, ee: &[u8]) -> Outcome {
    let h = match ImageHeader::parse(&ee[HDR..HDR + EE_PAGE]) {
        Ok(h) => h,
        Err(e) => {
            c.add(Level::Warn, "header", format!("{e}: boot1 enters rescue mode"));
            return Outcome::Rescue(e.to_string());
        }
    };
    if h.body_len < 12 || h.body_len as usize > BODY.len() || h.body_len % 4 != 0 || !APP_RAM.contains(&h.entry) {
        c.add(Level::Error, "header", "length or entry out of range");
        return Outcome::Rescue("bad header".into());
    }
    c.add(Level::Info, "header", format!("seq {}, body {} bytes, entry 0x{:08x}, flags 0x{:x}, fw 0x{:08x}", h.seq, h.body_len, h.entry, h.flags, h.fw_version));
    let b = &ee[BODY.start..BODY.start + h.body_len as usize];
    if crc32fast::hash(b) != h.body_crc {
        c.add(Level::Error, "body", "CRC-32 mismatch (interrupted update?): boot1 enters rescue mode");
        return Outcome::Rescue("bad body CRC".into());
    }
    match body::parse(b) {
        Err(e) => {
            c.add(Level::Error, "body", e.clone());
            return Outcome::Rescue(e);
        }
        Ok(l) if l.entry != h.entry => {
            c.add(Level::Error, "body", "Jump_Close target differs from the header entry");
            return Outcome::Rescue("entry mismatch".into());
        }
        Ok(l) => c.add(Level::Info, "body", format!("{} LZ4 sections, {} bytes loaded", l.sections.len(), l.sections.iter().map(|s| s.data.len()).sum::<usize>())),
    }
    if h.flags & HF_TRIAL != 0 {
        let st = BootState::parse(&ee[STATE..STATE + 16]).filter(|s| s.seq == h.seq);
        match st {
            Some(s) if s.confirmed => c.add(Level::Info, "state", "image confirmed"),
            Some(s) if s.attempts >= BOOT_TRIALS => {
                c.add(Level::Warn, "state", "trial exhausted: boot1 stays in rescue");
                return Outcome::Rescue("trial exhausted".into());
            }
            s => c.add(Level::Info, "state", format!("trial image, {} unconfirmed start(s)", s.map_or(0, |s| s.attempts))),
        }
    }
    Outcome::Boots { entry: h.entry, seq: h.seq }
}

fn check_profiles(c: &mut Ctx, ee: &[u8]) {
    let kind = |a: usize| match &ee[a..a + 4] {
        b"OCP1" | b"OCC1" => "ours",
        _ if ee[a..a + 16].iter().all(|&x| x == 0xFF) => "blank",
        _ => "foreign",
    };
    let mut foreign = 0;
    for slot in 0..4 {
        for copy in 0..2 {
            if kind(profile_copy(slot, copy)) == "foreign" {
                foreign += 1;
            }
        }
    }
    foreign += CONFIG_COPIES.iter().filter(|&&a| kind(a) == "foreign").count();
    if foreign > 0 {
        c.add(Level::Warn, "profiles", format!("{foreign} copies hold foreign data: the firmware will not write them (erase the area)"));
    }
}

pub fn verify(ee: &[u8], exp: &Expected) -> Report {
    let mut c = Ctx { f: Vec::new() };
    if ee.len() != EE_SIZE {
        c.add(Level::Error, "dump", format!("{} bytes, expected {EE_SIZE}", ee.len()));
        return Report { findings: c.f, outcome: Outcome::NoBootChain { stock: false } };
    }
    if !check_boot0(&mut c, &ee[BOOT0], exp.boot0) {
        let stock = le32(ee, 0) == MAGIC && le32(ee, 4) == 0x5853_5904;
        let msg = if stock { "stock Bose boot0: not migrated" } else { "boot0 is not a valid boot0: the DSP cannot boot" };
        c.add(if stock { Level::Warn } else { Level::Error }, "boot0", msg);
        return Report { findings: c.f, outcome: Outcome::NoBootChain { stock } };
    }
    if !check_boot1(&mut c, &ee[BOOT1], exp.boot1) {
        return Report { findings: c.f, outcome: Outcome::NoBootChain { stock: false } };
    }
    let outcome = check_image(&mut c, ee);
    check_profiles(&mut c, ee);
    match GraphData::parse(ee) {
        Ok(g) => c.add(Level::Info, "graph", format!("graph data pages: {} bytes for RAM 0x{:08x}, CRC OK", g.payload.len(), g.addr)),
        Err(e) => c.add(Level::Warn, "graph", format!("{e}: \"Bose d'origine\" falls back to the EQ path (layout v3 firmware)")),
    }
    c.add(Level::Info, "last page", format!("byte 0xFFFF = 0x{:02x} (stock CKT value)", ee[EE_SIZE - 1]));
    Report { findings: c.f, outcome }
}
