mod ais_cmd;
mod backup_cmd;
mod base_cmd;
mod device_cmd;
mod dsp_cmd;
mod firmware_cmd;
mod install;
mod tas_cmd;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "oc", about = "OpenCompanion tools for the Bose Companion 5")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// DSP AIS images.
    Ais {
        #[command(subcommand)]
        cmd: ais_cmd::AisCmd,
    },
    /// DSP versions and status (read-only).
    Probe,
    /// Dump the TAS EEPROM over USB (read-only).
    DumpTas {
        #[arg(short, long, default_value = "u6002_tas_usb.bin")]
        output: std::path::PathBuf,
    },
    /// DSP tools.
    Dsp {
        #[command(subcommand)]
        cmd: dsp_cmd::DspCmd,
    },
    /// TAS1020B USB controller tools.
    Tas {
        #[command(subcommand)]
        cmd: tas_cmd::TasCmd,
    },
    /// Back up both EEPROMs (read from the speaker, nothing written) into a new timestamped folder.
    Backup(backup_cmd::BackupArgs),
    /// Installation and update chain (status, DSP, TAS, restore).
    Install {
        #[command(subcommand)]
        cmd: install::InstallCmd,
    },
    /// Parametric base program: export from the stock backup, check (offline).
    Base {
        #[command(subcommand)]
        cmd: base_cmd::BaseCmd,
    },
    /// Firmware release assets (zip, sha256, manifest) for GitHub.
    Firmware {
        #[command(subcommand)]
        cmd: firmware_cmd::FirmwareCmd,
    },
}

fn main() -> anyhow::Result<()> {
    // Files of the earlier storage layout move to the current one (idempotent; the app does the same).
    if let Ok(moved) = oc_install::paths::migrate() {
        for m in moved {
            eprintln!("moved {m}");
        }
    }
    match Cli::parse().cmd {
        Cmd::Ais { cmd } => ais_cmd::run(cmd),
        Cmd::Probe => device_cmd::probe(),
        Cmd::DumpTas { output } => device_cmd::dump_tas(&output),
        Cmd::Dsp { cmd } => dsp_cmd::run(cmd),
        Cmd::Tas { cmd } => tas_cmd::run(cmd),
        Cmd::Install { cmd } => install::run(cmd),
        Cmd::Backup(a) => backup_cmd::run(a),
        Cmd::Base { cmd } => base_cmd::run(cmd),
        Cmd::Firmware { cmd } => firmware_cmd::run(cmd),
    }
}
