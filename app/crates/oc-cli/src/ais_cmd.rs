use std::path::PathBuf;

use anyhow::Context;
use clap::Subcommand;
use oc_ais::{BuildOptions, Command};

const RAM_BASE: u32 = 0x1000_0000;

#[derive(Subcommand)]
pub enum AisCmd {
    /// Describe an AIS image.
    Info { image: PathBuf },
    /// Produce the flat RAM image.
    Unpack {
        image: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Convert a TI ELF to an AIS image (uncompressed sections).
    Pack {
        elf: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

pub fn run(cmd: AisCmd) -> anyhow::Result<()> {
    match cmd {
        AisCmd::Info { image } => info(&image),
        AisCmd::Unpack { image, output } => {
            let img = oc_ais::parse(&std::fs::read(&image)?)?;
            let flat = img.flatten(RAM_BASE);
            std::fs::write(&output, &flat)?;
            println!("{}: {} bytes from 0x{RAM_BASE:08x}", output.display(), flat.len());
            Ok(())
        }
        AisCmd::Pack { elf, output } => {
            let data = std::fs::read(&elf).with_context(|| format!("reading {}", elf.display()))?;
            let (segs, fills, entry) = oc_ais::segments_from_elf(&data)?;
            let opts = BuildOptions::default();
            let img = oc_ais::build(&segs, &fills, entry, &opts)?;
            let bytes = oc_ais::write(&img)?;
            anyhow::ensure!(bytes.len() <= opts.max_size, "image too large: {} bytes (max {})", bytes.len(), opts.max_size);
            std::fs::write(&output, &bytes)?;
            println!("{}: {} bytes, {} segments, entry 0x{entry:08x}", output.display(), bytes.len(), segs.len());
            Ok(())
        }
    }
}

fn info(path: &PathBuf) -> anyhow::Result<()> {
    let data = std::fs::read(path)?;
    let img = oc_ais::parse(&data)?;
    for c in &img.commands {
        match c {
            Command::SectionLoad(s) => println!("load     0x{:08x} {:7} B", s.addr, s.data.len()),
            Command::BoseZlib(s) => println!("zlib     0x{:08x} {:7} B (inflated)", s.addr, s.data.len()),
            Command::SectionFill { addr, size, pattern, .. } => println!("fill     0x{addr:08x} {size:7} B pattern 0x{pattern:08x}"),
            Command::Set { kind, addr, data, .. } => println!("set      0x{addr:08x} = 0x{data:08x} (type {kind})"),
            Command::Jump(a) => println!("call     0x{a:08x}"),
            Command::JumpClose(a) => println!("start    0x{a:08x}"),
            other => println!("{other:?}"),
        }
    }
    println!("{} commands, entry {:x?}", img.commands.len(), img.entry());
    Ok(())
}
