use std::path::PathBuf;

use anyhow::bail;
use clap::Args;
use oc_install::backup::{DSP_FILE, TAS_FILE};
use oc_install::restore::{reboot_tas, restore_dsp, restore_tas, DspRestoreInputs, DspRestoreOptions};
use oc_install::tas::{OcPort, StockPort, TasOptions};

use super::connect::Conn;
use super::host::CliHost;
use super::loaders::LoaderArgs;
use super::source::stock_backup;

#[derive(Args)]
pub struct RestoreArgs {
    /// Only the DSP (U7001).
    #[arg(long, conflicts_with_all = ["tas", "all"])]
    dsp: bool,
    /// Only the USB controller (U6002).
    #[arg(long, conflicts_with = "all")]
    tas: bool,
    /// Both chips, DSP first (the default).
    #[arg(long)]
    all: bool,
    /// Every read and check, no EEPROM write.
    #[arg(long)]
    dry_run: bool,
    /// Backup folder to restore (default: the first stock backup under `--root`).
    #[arg(long)]
    from: Option<PathBuf>,
    /// Root of the backups (default: the app's, in the OS data directory).
    #[arg(long)]
    root: Option<PathBuf>,
    /// Do not restart the USB controller at the end (power-cycle the speaker instead).
    #[arg(long)]
    no_reboot: bool,
    #[command(flatten)]
    loaders: LoaderArgs,
}

pub fn run(a: RestoreArgs) -> anyhow::Result<()> {
    let (do_dsp, do_tas) = match (a.dsp, a.tas) {
        (true, _) => (true, false),
        (_, true) => (false, true),
        _ => (true, true),
    };
    let files = stock_backup(a.from.as_deref(), a.root.as_deref())?;
    let conn = Conn::open_required()?;
    let mut host = CliHost::default();
    if do_dsp {
        let Some(image) = files.dsp.clone() else { bail!("the backup has no {DSP_FILE}") };
        let (stub, updater) = a.loaders.load()?;
        let img = DspRestoreInputs { image, stub, updater };
        let rep = conn.with_dsp(|l| restore_dsp(l, &img, &mut host, DspRestoreOptions { dry_run: a.dry_run }))?;
        if rep.restored {
            println!("DSP restored: {} (U7001 CRC-32 0x{:08x})", rep.after, rep.crc32.unwrap_or(0));
        }
    }
    if do_tas {
        let Some(image) = files.tas.as_deref() else { bail!("the backup has no {TAS_FILE}") };
        let opts = TasOptions { dry_run: a.dry_run };
        let rep = match &conn {
            Conn::Bose(c) => restore_tas(&StockPort::new(c), image, &mut host, opts)?,
            Conn::Oc(d) => restore_tas(&OcPort::new(d), image, &mut host, opts)?,
        };
        if rep.restored && rep.pages_written > 0 {
            match &conn {
                Conn::Oc(d) if !a.no_reboot => reboot_tas(d, &mut host)?,
                _ => println!("power-cycle the speaker to start the Bose USB controller firmware."),
            }
        }
    }
    Ok(())
}
