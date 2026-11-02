use super::plan::{plan, TasPlan};
use super::port::TasPort;
use super::state::{classify, TasContent};
use super::target::{TasTarget, EE_SIZE, HEADER_LEN, PAGE};
use crate::error::{Error, Result};
use crate::host::{Host, Plan};

#[derive(Debug, Clone, Copy, Default)]
pub struct TasOptions {
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasReport {
    pub before: TasContent,
    pub pages_written: usize,
    pub installed: bool,
}

fn describe(p: &TasPlan, port: &dyn TasPort, target: &TasTarget) -> Plan {
    let mut writes = vec![format!(
        "U6002 0x0000..0x0011 (header, 18 bytes): DataType := 3 \"download in progress\" (via {})",
        port.name()
    )];
    if let (Some(first), Some(last)) = (p.writes.first(), p.writes.last()) {
        writes.push(format!(
            "U6002 0x{:04x}..0x{:04x}: {} page write(s) of at most {PAGE} bytes, {} bytes of code/padding/data area, each read back",
            first.0,
            last.0 + last.1.len() - 1,
            p.writes.len(),
            p.bytes()
        ));
    }
    writes.push(format!(
        "U6002 0x0000..0x0011: final header {} (DataType 1 written last)",
        target.header.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
    ));
    Plan {
        title: format!("TAS EEPROM install: {}", target.describe()),
        writes,
        warnings: vec![
            "Do not unplug the speaker until the end. If its power is cut between the first and the last header \
             write, the TAS ROM starts in DFU mode at the next power-up (recovery: opencompanion/docs/recovery.md)."
                .into(),
            "The new TAS firmware starts at the next power cycle of the speaker.".into(),
        ],
    }
}

fn apply(port: &dyn TasPort, p: &TasPlan, current: &[u8], target: &TasTarget, host: &mut dyn Host) -> Result<usize> {
    host.log("marking the TAS EEPROM header \"download in progress\" (DataType 3)…");
    port.begin(&current[..HEADER_LEN])?;
    for (i, (addr, data)) in p.writes.iter().enumerate() {
        port.write(*addr, data)?;
        if (i + 1) % 32 == 0 || i + 1 == p.writes.len() {
            host.log(&format!("  code {}/{} pages", i + 1, p.writes.len()));
        }
    }
    host.log("writing the final header (DataType 1 last)…");
    port.commit(&target.header)?;
    Ok(p.writes.len())
}

/// Safe to re-run after any interruption.
pub fn install(port: &dyn TasPort, target: &TasTarget, host: &mut dyn Host, opts: TasOptions) -> Result<TasReport> {
    host.log(&format!("reading the TAS EEPROM through the {}…", port.name()));
    let current = port.read(0, EE_SIZE)?;
    let before = classify(&current, Some(target));
    host.log(&format!("current content: {before:?}"));
    if before == TasContent::Target {
        host.log("the target image is already installed (whole EEPROM verified).");
        return Ok(TasReport { before, pages_written: 0, installed: true });
    }
    if before == TasContent::Invalid {
        return Err(Error::Refused("the TAS EEPROM has no valid boot header: not touching it".into()));
    }
    let p = plan(&current, target, &port.writable())?;
    let summary = describe(&p, port, target);
    for line in summary.lines() {
        host.log(&line);
    }
    if opts.dry_run {
        host.log("dry run: nothing written.");
        return Ok(TasReport { before, pages_written: 0, installed: false });
    }
    if !host.confirm(&summary) {
        return Err(Error::Cancelled);
    }
    host.keep_awake(true);
    let written = apply(port, &p, &current, target, host);
    host.keep_awake(false);
    let written = written?;

    host.log("verifying the whole TAS EEPROM…");
    let after = port.read(0, EE_SIZE)?;
    if let Some(at) = after.iter().zip(&target.bytes).enumerate().position(|(i, (a, b))| a != b && i != super::plan::MODE_ADDR) {
        return Err(Error::Verify(format!("TAS EEPROM differs from the image at 0x{at:04x} (re-run to resume)")));
    }
    host.log("TAS EEPROM verified. Power-cycle the speaker to start the new TAS firmware.");
    Ok(TasReport { before, pages_written: written, installed: true })
}
