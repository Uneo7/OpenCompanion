use std::time::{Duration, Instant};

use oc_protocol::dsp::DspConsole;
use oc_protocol::Link;
use oc_ramload as rl;

use super::revision::{self, StockRevision};
use super::runtime::{detect, DspRuntime};
use super::updater::Updater;
use crate::error::{refused, Error, Result};
use crate::host::{Host, Plan};

/// Reference for the RAM-loader area (stock `.cinit` table) checked before the hijack. Digests are CRC-32s,
/// not firmware bytes: nothing of the Bose firmware is distributed.
#[derive(Debug, Clone, Copy)]
pub enum StockRef<'a> {
    /// The user's stock backup of this speaker: the digests are computed from it.
    Image(&'a [u8]),
    /// The digests of each revision table entry (the one whose layout the speaker's RAM matches): before the
    /// first backup.
    Known,
    /// One CRC-32 per 16-byte sample at `LOADER_LO + 0x100 * i` (see [`loader_digests`]).
    Digests(&'a [u32]),
}

/// Stock DSP firmware D01.01.01; checked against a real dump by `tests/real_inputs.rs`.
pub const STOCK_LOADER_DIGESTS: [u32; SAMPLES] = [
    0xf59fcb2c, 0xc1df2867, 0xbbc8f80c, 0xa08ca714, 0xdfa46c4a, 0x1184ce33,
    0xb264e638, 0xecbb4b55, 0xaf50d170, 0x2fdb1782, 0x4df3f8ff, 0x5792815a,
    0x0397bf14, 0x010c0be9, 0x82d5be60, 0x4fc2e4c7, 0x4853e8ab, 0x6994a7c8,
    0x9a9c48c3, 0x586a593c, 0x7dc891b4, 0xbeed56e2, 0xdf89b620, 0xa64e7750,
    0xee82f84b, 0xd97696ab, 0xb53595d3, 0xc88cb320, 0x184b7129, 0x0e35790b,
];

pub const SAMPLES: usize = ((rl::LOADER_HI - rl::LOADER_LO) / 0x100) as usize;

/// The stock image bytes equal what the stock firmware's RAM holds there after boot (D01.01.01 loader area).
pub fn loader_digests(stock_image: &[u8]) -> Result<Vec<u32>> {
    revision_digests(stock_image, &revision::D010101)
}

/// [`loader_digests`] over the loader area of `rev`.
pub fn revision_digests(stock_image: &[u8], rev: &StockRevision) -> Result<Vec<u32>> {
    let ram = rl::stock_ram(stock_image)?;
    Ok(rev.samples().map(|a| crc32fast::hash(&ram[off(a)..off(a) + 16])).collect())
}

fn off(addr: u32) -> usize {
    (addr - rl::RAM_LO) as usize
}

#[derive(Debug, Clone, Copy)]
pub struct Loaders<'a> {
    pub stock: StockRef<'a>,
    pub stub: &'a rl::LoadImage,
    pub updater: &'a rl::LoadImage,
}

pub(crate) const WAIT_RUNTIME: Duration = Duration::from_secs(30);

/// Budget counted both in wall-clock time (each [`detect`] may take seconds on a silent DSP) and in 500 ms
/// polls (links whose delays are simulated).
pub(crate) fn wait_runtime(link: &dyn Link, budget: Duration) -> DspRuntime {
    let deadline = Instant::now() + budget;
    let polls = (budget.as_millis() / 500).max(1);
    let mut rt = DspRuntime::Silent("not asked".into());
    for _ in 0..polls {
        if Instant::now() >= deadline {
            break;
        }
        link.delay(Duration::from_millis(500));
        rt = detect(link);
        if !matches!(rt, DspRuntime::Silent(_)) {
            break;
        }
    }
    rt
}

/// The loader area must hold the stock `.cinit` table (dead after boot), sampled twice one second apart, like
/// `oc_ramload::check_area_free`, except that samples already holding our stub are accepted too: an earlier
/// run interrupted during the stub upload (stock firmware still running) leaves part of it there. With
/// [`StockRef::Digests`] a sample only partly covered by the stub cannot be checked: it is refused (a power
/// cycle of the speaker restores the stock RAM).
fn check_loader_area<L: Link + ?Sized>(console: &DspConsole<L>, b: &Loaders, rev: &StockRevision) -> Result<()> {
    // Every byte the stub upload writes must lie in the area checked for this revision.
    for (a, d) in &b.stub.segments {
        if *a < rl::LOADER_LO || *a + d.len() as u32 > rev.loader_hi {
            return refused(format!(
                "stub segment 0x{a:08x} +0x{:x} is outside the loader area checked on {} (0x{:08x}..0x{:08x})",
                d.len(),
                rev.dsp,
                rl::LOADER_LO,
                rev.loader_hi
            ));
        }
    }
    let digests = match b.stock {
        StockRef::Image(img) => revision_digests(img, rev)?,
        StockRef::Known => rev.digests.to_vec(),
        StockRef::Digests(d) => d.to_vec(),
    };
    let samples = rev.samples().count();
    if digests.len() != samples {
        return refused(format!("{} loader-area digests, expected {samples} for {}", digests.len(), rev.dsp));
    }
    let stock_ram = match b.stock {
        StockRef::Image(img) => Some(rl::stock_ram(img)?),
        StockRef::Known | StockRef::Digests(_) => None,
    };
    // Sample with the stub overlaid: exact with the image; with digests only where the stub covers it all.
    let with_stub = |addr: u32| -> Option<Vec<u8>> {
        let mut out: Vec<Option<u8>> = match &stock_ram {
            Some(r) => r[off(addr)..off(addr) + 16].iter().map(|&x| Some(x)).collect(),
            None => vec![None; 16],
        };
        for (a, d) in &b.stub.segments {
            for (i, o) in out.iter_mut().enumerate() {
                let at = addr + i as u32;
                if at >= *a && at < *a + d.len() as u32 {
                    *o = Some(d[(at - a) as usize]);
                }
            }
        }
        out.into_iter().collect()
    };
    for attempt in 0..2 {
        for (i, addr) in rev.samples().enumerate() {
            let got: Vec<u8> = console.read_words(addr, 4)?.iter().flat_map(|w| w.to_le_bytes()).collect();
            if crc32fast::hash(&got) != digests[i] && Some(&got) != with_stub(addr).as_ref() {
                return refused(format!(
                    "RAM loader area at 0x{addr:08x} holds neither the stock .cinit table nor our stub: {got:02x?}"
                ));
            }
        }
        if attempt == 0 {
            console.link().delay(Duration::from_secs(1));
        }
    }
    Ok(())
}

fn not_hijacked(svm: &str, e: Error) -> Error {
    match e {
        Error::Cancelled => e,
        e => Error::Refused(format!(
            "the RAM loader could not take over this Bose DSP firmware ({svm}): {e}. Nothing was written: the speaker \
             keeps its firmware (if it stopped answering, unplug it from the mains for 10 s)"
        )),
    }
}

/// Read-only: the table entry whose layout the speaker's RAM holds (hijack signature, then the loader area).
/// Chosen by content, never by the version string: a later Bose revision with a known layout works as is.
fn match_layout<L: Link + ?Sized>(console: &DspConsole<L>, b: &Loaders) -> Result<&'static StockRevision> {
    let mut why = Vec::new();
    for rev in &revision::REVISIONS {
        let v = console.read_words(rev.flags_addr(), 2)?;
        if v[0] != 0x3 || v[1] != rev.process {
            why.push(format!("{} layout: hijack target flags 0x{:x}, process 0x{:08x}", rev.dsp, v[0], v[1]));
            continue;
        }
        match check_loader_area(console, b, rev) {
            Ok(()) => return Ok(rev),
            Err(Error::Refused(e)) => why.push(format!("{} layout: {e}", rev.dsp)),
            Err(e) => return Err(e),
        }
    }
    refused(format!("this Bose firmware's RAM matches no known layout ({})", why.join("; ")))
}

fn hijack(link: &dyn Link, b: &Loaders, host: &mut dyn Host) -> Result<()> {
    let console = DspConsole::new(link);
    host.log("M1: matching the stock RAM layout (read-only: hijack target, loader area)…");
    let rev = match_layout(&console, b)?;
    host.log(&format!("M1: the stock RAM has the {} layout.", rev.dsp));
    host.log("M1: installing the RAM loader stub (BWW, verified with BRR)…");
    rl::install_stub(&console, &b.stub.segments, |l| host.log(&l))?;
    host.log("M1: hijacking the output meter…");
    rl::hijack_with(&console, rev.module, rev.process, b.stub.entry, |l| host.log(&l))?;
    let v = rl::probe(link)?;
    host.log(&format!("M1: RAM loader running (v{v})."));
    Ok(())
}

fn load_updater(link: &dyn Link, b: &Loaders, host: &mut dyn Host) -> Result<DspRuntime> {
    host.log("loading the updater into DSP RAM…");
    rl::load_image(link, b.updater, |l| host.log(&l))?;
    Ok(wait_runtime(link, WAIT_RUNTIME))
}

/// From the OpenCompanion firmware: `RSC` (rescue flag in RAM, no EEPROM write); if RAM is not retained
/// across the reset, `UPD` (the firmware zeroes the header page), only with the user's consent.
fn leave_firmware(link: &dyn Link, host: &mut dyn Host, dry_run: bool) -> Result<DspRuntime> {
    host.log("asking the OpenCompanion DSP firmware for rescue mode (RSC, no EEPROM write)…");
    let _ = link.transact(b'E', b"RSC\r", Duration::from_millis(1000));
    let rt = wait_runtime(link, WAIT_RUNTIME);
    if matches!(rt, DspRuntime::Rescue { .. }) {
        return Ok(rt);
    }
    // UPD needs our firmware answering: a silent DSP is an error (nothing was written, resumable).
    if let DspRuntime::Silent(e) = &rt {
        return Err(Error::Device(format!(
            "the DSP did not come back after RSC within {} s ({e}); unplug the speaker from the mains for 10 s, then resume",
            WAIT_RUNTIME.as_secs()
        )));
    }
    if dry_run {
        return refused(format!(
            "RSC did not reach rescue mode (now: {rt}); reading U7001 would need UPD, which invalidates the image \
             header: not done in a dry run"
        ));
    }
    let plan = Plan {
        title: "DSP firmware → rescue mode".into(),
        writes: vec!["U7001 0x0100..0x017F image header := zeros, by the DSP firmware (UPD)".into()],
        warnings: vec!["The DSP then stays in rescue mode until the EEPROM work completes.".into()],
    };
    if !host.confirm(&plan) {
        return Err(Error::Cancelled);
    }
    let _ = link.transact(b'E', b"UPD\r", Duration::from_millis(1000));
    Ok(wait_runtime(link, WAIT_RUNTIME))
}

pub(crate) fn enter_updater(link: &dyn Link, b: &Loaders, host: &mut dyn Host, dry_run: bool) -> Result<DspRuntime> {
    let before = detect(link);
    host.log(&format!("DSP: {before}"));
    let mut rt = before.clone();
    for _ in 0..4 {
        rt = match rt {
            DspRuntime::Updater { .. } => return Ok(before),
            DspRuntime::Stock { svm } => {
                // The layout is matched on the speaker's RAM (match_layout), the version only goes to the log.
                // Only RAM is touched up to here: a failure leaves the EEPROMs as they were.
                hijack(link, b, host).and_then(|()| load_updater(link, b, host)).map_err(|e| not_hijacked(&svm, e))?
            }
            DspRuntime::Stub { .. } | DspRuntime::Rescue { .. } => load_updater(link, b, host)?,
            DspRuntime::OcFirmware { .. } => leave_firmware(link, host, dry_run)?,
            DspRuntime::OtherFirmware { svm } => return refused(format!("unknown DSP firmware ({svm})")),
            DspRuntime::Silent(e) => return Err(Error::Device(format!("the DSP does not answer: {e}"))),
        };
        host.log(&format!("DSP: {rt}"));
    }
    Err(Error::Device(format!("could not reach the updater (DSP: {rt})")))
}

/// After a full read-back verify and a reboot: `after` is what answered. A DSP that does not answer properly
/// (a stale or garbled relay reply reads as [`DspRuntime::Silent`]) is asked again for ~10 s; still silent, it
/// is accepted with a warning when the relay still returns a frame (garbled): the EEPROM content was already
/// verified byte for byte, so the flow goes on. A failing link (no frame at all) is never accepted.
/// A definite other answer (e.g. rescue mode) is not accepted. Returns what answered and whether to go on.
pub(crate) fn settle_after_verify(link: &dyn Link, after: DspRuntime, expected: impl Fn(&DspRuntime) -> bool, what: &str, host: &mut dyn Host) -> (DspRuntime, bool) {
    if expected(&after) {
        return (after, true);
    }
    let after = if matches!(after, DspRuntime::Silent(_)) { wait_runtime(link, SETTLE_AFTER_VERIFY) } else { after };
    if expected(&after) {
        return (after, true);
    }
    // Only a garbled reply is accepted: the relay must still carry a frame back. A link that fails (transport
    // error, host crash, power loss) is a real interruption and fails the flow.
    let answers = matches!(link.transact(b'E', b"SVM\r", Duration::from_millis(1000)), Ok((_, d)) if !d.starts_with(b"ERROR"));
    if let (DspRuntime::Silent(e), true) = (&after, answers) {
        host.log(&format!(
            "WARNING: {what} did not answer properly after the reboot ({e}); the EEPROM was verified byte for byte before it, so the flow goes on"
        ));
        return (after, true);
    }
    (after, false)
}

/// How long [`settle_after_verify`] keeps asking a silent DSP.
pub(crate) const SETTLE_AFTER_VERIFY: Duration = Duration::from_secs(10);

/// Reboots the DSP from the updater (RTI reset: the ROM boots from U7001) and reports what comes up.
pub(crate) fn reboot(u: &Updater, host: &mut dyn Host) -> DspRuntime {
    if let Err(e) = u.reboot() {
        host.log(&format!("REBOOT: {e}"));
    }
    let rt = wait_runtime(u.link(), WAIT_RUNTIME);
    host.log(&format!("DSP after reboot: {rt}"));
    rt
}
