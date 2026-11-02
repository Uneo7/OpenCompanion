use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use clap::Args;
use oc_ais::eeprom::boot::{boot0_from_elf, boot1_from_elf};
use oc_ais::eeprom::image::{app_image, AppOptions};
use oc_install::dsp::{install, DspBundle, DspOptions};

use super::connect::Conn;
use super::host::CliHost;
use super::loaders::LoaderArgs;
use super::source::stock_backup;

#[derive(Args)]
pub struct DspArgs {
    /// Every read and check (the updater is loaded into DSP RAM), no EEPROM write; the DSP is rebooted after.
    #[arg(long)]
    dry_run: bool,
    /// Backup folder holding this speaker's stock U7001 (default: its first stock backup, see `oc backup`).
    /// The install refuses to start without a verified backup read from the speaker.
    #[arg(long)]
    from: Option<PathBuf>,
    /// Root of the backups (default: the app's, in the OS data directory).
    #[arg(long)]
    root: Option<PathBuf>,
    /// OpenCompanion DSP firmware ELF.
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/dsp/build/opencompanion_dsp.out"))]
    app: PathBuf,
    /// Firmware version stored in the image header (informative).
    #[arg(long, default_value_t = 0, value_parser = parse_u32)]
    fw_version: u32,
    /// Trial image (boot1 falls back to rescue after 2 unconfirmed starts): only for a self-confirming firmware.
    #[arg(long)]
    trial: bool,
    #[command(flatten)]
    loaders: LoaderArgs,
}

fn parse_u32(s: &str) -> Result<u32, String> {
    match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16),
        None => s.parse(),
    }
    .map_err(|e| e.to_string())
}

fn read(p: &Path) -> anyhow::Result<Vec<u8>> {
    std::fs::read(p).with_context(|| format!("reading {}", p.display()))
}

fn bundle(a: &DspArgs) -> anyhow::Result<DspBundle> {
    let backup = stock_backup(a.from.as_deref(), a.root.as_deref())?;
    let Some(stock) = backup.dsp else { bail!("the backup has no U7001 dump") };
    let boot = a.loaders.boot_dir();
    let (stub, updater) = a.loaders.load()?;
    let opts = AppOptions { seq: 0, fw_version: a.fw_version, trial: a.trial, allow_legacy_app: false };
    let b = DspBundle {
        stock,
        stub,
        updater,
        boot0: boot0_from_elf(&read(&boot.join("tramp.out"))?).context("tramp.out")?,
        boot1: boot1_from_elf(&read(&boot.join("boot1.out"))?).context("boot1.out")?,
        app: app_image(&read(&a.app)?, &opts).context("firmware image")?,
    };
    b.validate()?;
    Ok(b)
}

pub fn run(a: DspArgs) -> anyhow::Result<()> {
    let b = bundle(&a)?;
    println!(
        "firmware: body {} bytes (CRC 0x{:08x}), entry 0x{:08x}, fw 0x{:08x}{}",
        b.app.body.bytes.len(),
        b.app.header.body_crc,
        b.app.header.entry,
        b.app.header.fw_version,
        if a.trial { ", trial" } else { "" }
    );
    let conn = Conn::open_required()?;
    let mut host = CliHost::default();
    let rep = conn.with_dsp(|link| install(link, &b, &mut host, DspOptions { dry_run: a.dry_run }))?;
    if rep.installed {
        println!("done: {}", rep.after);
    } else if !a.dry_run {
        bail!("not installed (DSP: {})", rep.after);
    }
    Ok(())
}
