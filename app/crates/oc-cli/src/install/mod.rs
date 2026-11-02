pub(crate) mod connect;
mod dsp;
pub(crate) mod host;
pub(crate) mod loaders;
mod restore;
mod source;
mod status;
mod tas;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum InstallCmd {
    /// Detect what is installed on each chip (stock / OpenCompanion / rescue / DFU), reads only.
    Status,
    /// Install or update the DSP firmware and its boot chain in U7001 (from the stock firmware: M1..M5).
    Dsp(dsp::DspArgs),
    /// Install or update the TAS firmware in U6002.
    Tas(tas::TasArgs),
    /// Back to the stock Bose firmware from the user's own backup (DSP first, then the TAS).
    Restore(restore::RestoreArgs),
}

pub fn run(cmd: InstallCmd) -> anyhow::Result<()> {
    match cmd {
        InstallCmd::Status => status::run(),
        InstallCmd::Dsp(a) => dsp::run(a),
        InstallCmd::Tas(a) => tas::run(a),
        InstallCmd::Restore(a) => restore::run(a),
    }
}
