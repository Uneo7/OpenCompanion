use std::path::PathBuf;

use clap::Args;
use oc_install::backup::{default_root, read_dsp, read_tas, save, speaker_id, stamp_now};
use oc_install::dsp::{Loaders, StockRef, STOCK_LOADER_DIGESTS};
use oc_install::tas::{OcPort, StockPort};

use crate::install::connect::Conn;
use crate::install::host::CliHost;
use crate::install::loaders::LoaderArgs;

#[derive(Args)]
pub struct BackupArgs {
    /// Root of the backups (default: the app's, in the OS data directory).
    #[arg(long)]
    out: Option<PathBuf>,
    #[command(flatten)]
    loaders: LoaderArgs,
}

pub fn run(a: BackupArgs) -> anyhow::Result<()> {
    let (stub, updater) = a.loaders.load()?;
    let root = a.out.unwrap_or_else(default_root);
    let conn = Conn::open_required()?;
    let mut host = CliHost::default();
    let tas = match &conn {
        Conn::Bose(c) => read_tas(&StockPort::new(c), &mut host)?,
        Conn::Oc(d) => read_tas(&OcPort::new(d), &mut host)?,
    };
    // The stock image is not known yet: the RAM-loader area is checked against built-in digests.
    let loaders = Loaders { stock: StockRef::Digests(&STOCK_LOADER_DIGESTS), stub: &stub, updater: &updater };
    let dsp = conn.with_dsp(|l| read_dsp(l, &loaders, &mut host))?;
    let e = save(&root, &speaker_id(&dsp.bytes), &stamp_now(), Some(&tas), Some(&dsp.bytes))?;
    for (name, d) in [("U6002", e.tas), ("U7001", e.dsp)] {
        if let Some(d) = d {
            println!("{name}: {} (CRC-32 0x{:08x})", d.content, d.crc32);
        }
    }
    println!("saved: {} (DSP now: {})", e.dir.display(), dsp.after);
    Ok(())
}
