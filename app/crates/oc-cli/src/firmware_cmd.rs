//! `oc firmware pack`: the two `.ocfw` containers (DSP, USB controller) of the build outputs, the only firmware
//! files the app installs. `oc firmware package` (feature `firmware-package`): the GitHub release assets.
//! Format: `docs/developer/firmware-releases.md`.

use std::path::{Path, PathBuf};

use clap::Subcommand;
use oc_device::install::pack::{sources_of_dir, write_pack};
use oc_device::install::FirmwareSources;

#[derive(Subcommand)]
pub enum FirmwareCmd {
    /// Wraps the build outputs into `opencompanion-dsp.ocfw` and `opencompanion-usb.ocfw` in `--out`.
    Pack {
        /// Firmware folder (raw files in the release layout, or the two `.ocfw`); default: the development tree.
        #[arg(long)]
        from: Option<PathBuf>,
        #[arg(long, default_value = "dist")]
        out: PathBuf,
        /// Oldest app version that can install this firmware.
        #[arg(long, default_value = env!("CARGO_PKG_VERSION"))]
        min_app: String,
    },
    /// Builds `opencompanion-firmware-<dsp>-<tas>.zip` (the two `.ocfw` + manifest), its `.sha256` and
    /// `firmware-manifest.json` into `--out`.
    #[cfg(feature = "firmware-package")]
    Package {
        #[arg(long)]
        from: Option<PathBuf>,
        #[arg(long, default_value = "dist")]
        out: PathBuf,
        #[arg(long, default_value = env!("CARGO_PKG_VERSION"))]
        min_app: String,
    },
}

fn sources(from: Option<PathBuf>) -> anyhow::Result<FirmwareSources> {
    Ok(match from {
        Some(d) => sources_of_dir(&d, env!("CARGO_PKG_VERSION")).map_err(anyhow::Error::msg)?,
        None => FirmwareSources::dev_tree(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))),
    })
}

pub fn run(cmd: FirmwareCmd) -> anyhow::Result<()> {
    match cmd {
        FirmwareCmd::Pack { from, out, min_app } => {
            let [d, u] = write_pack(&sources(from)?, &out, &min_app).map_err(anyhow::Error::msg)?;
            for p in [d, u] {
                let v = oc_device::install::ocfw::peek_version(&p).unwrap_or_default();
                println!("{} ({v})", p.display());
            }
            Ok(())
        }
        #[cfg(feature = "firmware-package")]
        FirmwareCmd::Package { from, out, min_app } => {
            std::fs::create_dir_all(&out)?;
            let a = oc_device::install::release::package::package(&sources(from)?, &out, &min_app).map_err(anyhow::Error::msg)?;
            println!("DSP {} · USB controller {}", a.manifest_data.dsp_version, a.manifest_data.tas_version);
            for p in [&a.zip, &a.zip_sha256, &a.manifest] {
                println!("{}", p.display());
            }
            Ok(())
        }
    }
}
