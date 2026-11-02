use std::path::PathBuf;

use anyhow::Context;
use clap::Subcommand;
use oc_protocol::dsp::DspConsole;
use oc_protocol::transport::Companion;
use oc_ramload as rl;

/// Default stub: built by `make` in `opencompanion/firmware/dsp/loader`.
const DEFAULT_STUB: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/dsp/loader/build/loader.out");
/// DEV-ONLY default: a stock DSP EEPROM dump in the developer's tree (never shipped), used to check that the
/// loader area still holds the stock `.cinit` table. Users pass their own backup (`oc backup`).
const DEFAULT_STOCK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../u7001_dsp.bin");

#[derive(Subcommand)]
pub enum DspCmd {
    /// Load and run a DSP image from RAM (no EEPROM write). Standby or unplugging returns to the Bose firmware.
    Ramload {
        /// Image to run: TI ELF (.out) or AIS. Without it, only install the stub and check it answers.
        #[arg(long)]
        image: Option<PathBuf>,
        /// Loader stub (ELF built in firmware/dsp/loader).
        #[arg(long, default_value = DEFAULT_STUB)]
        stub: PathBuf,
        /// Stock DSP EEPROM image (u7001_dsp.bin).
        #[arg(long, default_value = DEFAULT_STOCK)]
        stock: PathBuf,
        /// Print the plan, send nothing.
        #[arg(long)]
        dry_run: bool,
        /// Stub already running: go straight to loading.
        #[arg(long)]
        skip_stub: bool,
        /// Only ask a running stub for its version.
        #[arg(long)]
        probe: bool,
    },
}

pub fn run(cmd: DspCmd) -> anyhow::Result<()> {
    match cmd {
        DspCmd::Ramload { image, stub, stock, dry_run, skip_stub, probe } => ramload(image, stub, stock, dry_run, skip_stub, probe),
    }
}

fn ramload(
    image: Option<PathBuf>,
    stub: PathBuf,
    stock: PathBuf,
    dry_run: bool,
    skip_stub: bool,
    probe: bool,
) -> anyhow::Result<()> {
    let stub = rl::read_image(&stub)?;
    stub.check_ranges(rl::LOADER_LO, rl::LOADER_HI, None)?;
    let image = image.map(|p| rl::read_image(&p)).transpose()?;
    if let Some(img) = &image {
        img.check_ranges(rl::RAM_LO, rl::RAM_HI, Some((rl::LOADER_LO, rl::LOADER_HI)))?;
    }
    for line in rl::plan(&stub, image.as_ref()) {
        println!("{line}");
    }
    if dry_run {
        return Ok(());
    }

    let link = Companion::open()?;
    if !probe && !skip_stub {
        let console = DspConsole::new(&link);
        println!("checking that the loader area still holds the stock .cinit table…");
        let stock = rl::stock_ram(&std::fs::read(&stock).with_context(|| format!("reading {}", stock.display()))?)?;
        rl::check_area_free(&console, &stock)?;
        println!("installing the stub…");
        rl::install_stub(&console, &stub.segments, |l| println!("{l}"))?;
        println!("hijacking the output meter…");
        rl::hijack(&console, stub.entry, |l| println!("{l}"))?;
    }
    let version = rl::probe(&link)?;
    println!("loader running, version {version}");
    if let (Some(img), false) = (&image, probe) {
        rl::load_image(&link, img, |l| println!("{l}"))?;
        println!("done: the new firmware is running. Standby or unplugging returns to the Bose firmware.");
    }
    Ok(())
}
