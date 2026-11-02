use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Args;
use oc_ais::eeprom::boot::updater_from_elf;
use oc_ramload::{self as rl, LoadImage};

pub const FW: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/dsp");

#[derive(Args, Clone)]
pub struct LoaderArgs {
    /// Directory of the boot-chain build (tramp.out, boot1.out, updater.out).
    #[arg(long)]
    pub boot_build: Option<PathBuf>,
    /// RAM loader stub ELF.
    #[arg(long)]
    pub stub: Option<PathBuf>,
}

impl LoaderArgs {
    pub fn boot_dir(&self) -> PathBuf {
        self.boot_build.clone().unwrap_or_else(|| Path::new(FW).join("boot/build"))
    }

    pub fn load(&self) -> anyhow::Result<(LoadImage, LoadImage)> {
        let stub = self.stub.clone().unwrap_or_else(|| Path::new(FW).join("loader/build/loader.out"));
        let updater = self.boot_dir().join("updater.out");
        let elf = std::fs::read(&updater).with_context(|| format!("reading {}", updater.display()))?;
        updater_from_elf(&elf).context("updater.out")?; // build tag and RAM range
        Ok((rl::read_image(&stub)?, rl::read_image(&updater)?))
    }
}
