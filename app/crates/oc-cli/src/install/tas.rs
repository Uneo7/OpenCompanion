use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use oc_install::dsp::{detect, DspRuntime};
use oc_install::tas::{install, OcPort, StockPort, TasOptions, TasTarget};
use oc_tas_image::data_area::DataArea;
use oc_tas_image::Image;

use super::connect::Conn;
use super::host::CliHost;

#[derive(Args)]
pub struct TasArgs {
    /// TAS image: 18-byte header + code (`firmware/usb/build/oc_tas/oc_tas.dfu`).
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/usb/build/oc_tas/oc_tas.dfu"))]
    image: PathBuf,
    /// Data area of the same build (`firmware/usb/build/oc_tas/oc_tas_data.bin`), written at 0x1800: the
    /// OpenCompanion TAS firmware does not connect to USB without it.
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/usb/build/oc_tas/oc_tas_data.bin"))]
    data: PathBuf,
    /// Every read and check, no write.
    #[arg(long)]
    dry_run: bool,
}

pub fn run(a: TasArgs) -> anyhow::Result<()> {
    let bytes = std::fs::read(&a.image).with_context(|| format!("reading {}", a.image.display()))?;
    let blob = std::fs::read(&a.data).with_context(|| format!("reading {}", a.data.display()))?;
    let data = DataArea::parse(&blob).with_context(|| format!("{} is not a valid data-area blob", a.data.display()))?;
    let target = TasTarget::with_data(Image::parse(&bytes)?, data)?;
    println!("{}", target.describe());
    let conn = Conn::open_required()?;
    let mut host = CliHost::default();
    let opts = TasOptions { dry_run: a.dry_run };
    let rep = match &conn {
        Conn::Bose(link) => {
            // doc.md, decisions section: DSP first, TAS last.
            if let rt @ DspRuntime::Stock { .. } = detect(link) {
                println!(
                    "WARNING: the DSP still runs {rt}. The planned order is `oc install dsp` first, then the TAS."
                );
            }
            install(&StockPort::new(link), &target, &mut host, opts)?
        }
        Conn::Oc(dev) => install(&OcPort::new(dev), &target, &mut host, opts)?,
    };
    if rep.installed && rep.pages_written > 0 {
        println!("power-cycle the speaker to start the new TAS firmware.");
    }
    Ok(())
}
