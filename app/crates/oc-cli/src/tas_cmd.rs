use std::path::PathBuf;

use anyhow::{bail, ensure, Context};
use clap::{Subcommand, ValueEnum};
use oc_protocol::tas_eeprom;
use oc_protocol::transport::Companion;
use oc_install::tas::{write_data_area, OcPort, StockPort, TasPort, DATA_AREA};
use oc_tas_image::data_area::DataArea;
use oc_tas_image::{DataType, Header, Image, BOSE_HEADER, HEADER_LEN};

use crate::install::host::CliHost;

#[derive(Clone, Copy, ValueEnum)]
pub enum Mode {
    /// Decode the current EEPROM header.
    Show,
    /// DataType 2 ("device EEPROM"): the ROM boots in DFU mode and loads firmware into RAM.
    Dfu,
    /// DataType 1 (application): the ROM boots the code stored in the EEPROM (stock Bose).
    App,
}

#[derive(Subcommand)]
pub enum TasCmd {
    /// Show or switch the boot mode stored in the TAS EEPROM header (needs the Bose firmware running).
    Mode {
        #[arg(value_enum)]
        mode: Mode,
        /// Really write the EEPROM (otherwise dry run).
        #[arg(long)]
        yes: bool,
    },
    /// USB DFU operations on the TAS1020B ROM boot loader.
    Dfu {
        #[command(subcommand)]
        cmd: DfuCmd,
    },
    /// From DFU mode, go back to the stock Bose firmware (DFU download of the Bose image with the header
    /// written to the EEPROM).
    RestoreApp {
        /// Bose EEPROM dump (header + code): the user's own backup (`oc backup`). DEV-ONLY default (a file in
        /// the current directory; no Bose image is shipped).
        #[arg(long, default_value = "u6002_tas.bin")]
        image: PathBuf,
        /// Really download (otherwise dry run).
        #[arg(long)]
        yes: bool,
    },
    /// TAS image tools.
    Image {
        #[command(subcommand)]
        cmd: ImageCmd,
    },
    /// OpenCompanion data area of U6002 (0x1800..0x1FFF: USB descriptors and tables of our TAS firmware).
    Data {
        #[command(subcommand)]
        cmd: DataCmd,
    },
}

#[derive(Subcommand)]
pub enum DataCmd {
    /// Check a data-area blob (magic, length, CRC-32).
    Info {
        #[arg(default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/usb/build/oc_tas/oc_tas_data.bin"))]
        blob: PathBuf,
    },
    /// DEVELOPMENT: write ONLY the data area, through the stock Bose firmware (`T`), for RAM tests of our TAS
    /// firmware while the stock image stays installed. Never writes below 0x1800 (header and code untouched);
    /// idempotent, each page read back, whole area verified. Asks for confirmation.
    Write {
        #[arg(default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/usb/build/oc_tas/oc_tas_data.bin"))]
        blob: PathBuf,
        /// Read and plan only.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
pub enum DfuCmd {
    /// List DFU devices on the bus.
    List,
    /// Download a TAS image (header + code) to the ROM DFU.
    Download {
        image: PathBuf,
        /// Force a RAM-only download (clears the header-overwrite attribute).
        #[arg(long)]
        to_ram: bool,
        /// Device VID:PID in hex (default: any DFU device found, e.g. 05a7:1020 or ffff:fffe).
        #[arg(long, value_parser = parse_id)]
        id: Option<(u16, u16)>,
        /// Really download (otherwise dry run).
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum ImageCmd {
    /// Describe a TAS image or EEPROM dump.
    Info { image: PathBuf },
    /// Wrap raw 8051 code (loaded at 0x0000) with the Bose header (VID 05A7, PID 1020, application).
    Build {
        code: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// Image meant for a RAM-only DFU download (header-overwrite attribute cleared).
        #[arg(long)]
        to_ram: bool,
    },
}

fn parse_id(s: &str) -> Result<(u16, u16), String> {
    let (v, p) = s.split_once(':').ok_or("expected VID:PID")?;
    let hex = |x: &str| u16::from_str_radix(x, 16).map_err(|e| e.to_string());
    Ok((hex(v)?, hex(p)?))
}

pub fn run(cmd: TasCmd) -> anyhow::Result<()> {
    match cmd {
        TasCmd::Mode { mode, yes } => mode_cmd(mode, yes),
        TasCmd::Dfu { cmd: DfuCmd::List } => dfu_list(),
        TasCmd::Dfu { cmd: DfuCmd::Download { image, to_ram, id, yes } } => {
            let mut img = Image::parse(&std::fs::read(&image).with_context(|| format!("reading {}", image.display()))?)?;
            if to_ram {
                img.set_header_overwrite(false);
            }
            dfu_download(&img, id, yes)
        }
        TasCmd::RestoreApp { image, yes } => restore_app(&image, yes),
        TasCmd::Data { cmd: DataCmd::Info { blob } } => {
            let area = read_area(&blob)?;
            println!(
                "{}: layout {}, payload {} bytes, CRC-32 OK; occupies 0x{:04x}..0x{:04x} of 0x{:04x}..0x{:04x}",
                blob.display(),
                area.version,
                area.payload.len(),
                DATA_AREA.start,
                DATA_AREA.start + area.to_bytes().len() - 1,
                DATA_AREA.start,
                DATA_AREA.end - 1
            );
            Ok(())
        }
        TasCmd::Data { cmd: DataCmd::Write { blob, dry_run } } => {
            let area = read_area(&blob)?;
            match crate::install::connect::Conn::open()? {
                Some(crate::install::connect::Conn::Bose(link)) => {
                    let rep = write_data_area(&StockPort::new(&link), &area, &mut CliHost::default(), dry_run)?;
                    if rep.written {
                        println!("{} page(s) written. Next: `oc tas mode dfu --yes`, power-cycle, then load oc_tas.dfu to RAM.", rep.pages_written);
                    }
                }
                Some(crate::install::connect::Conn::Oc(dev)) => {
                    // Our firmware only writes inside an EEPROM session: EE_BEGIN sets DataType 3, so the session
                    // is closed with the stock Bose header (DataType 1, checksum valid): the EEPROM then boots the
                    // untouched stock code at the next power-up, and `oc_tas.py dfu` reaches the ROM DFU without it.
                    let port = OcPort::new(&dev);
                    if !dry_run {
                        let current = port.read(0, HEADER_LEN)?;
                        port.begin(&current)?;
                    }
                    let rep = write_data_area(&port, &area, &mut CliHost::default(), dry_run);
                    if !dry_run {
                        port.commit(&BOSE_HEADER).context("closing the EEPROM session with the stock header")?;
                        println!("EEPROM header restored to the stock Bose header (DataType 1).");
                    }
                    let rep = rep?;
                    if rep.written {
                        println!("{} page(s) written through the OpenCompanion firmware.", rep.pages_written);
                    }
                }
                None => bail!("no Companion HID interface found (stock or OpenCompanion 05A7:1020, OpenCompanion 0.4.x 1209:0001 / 1209:0002)"),
            }
            Ok(())
        }
        TasCmd::Image { cmd: ImageCmd::Info { image } } => {
            let img = Image::parse(&std::fs::read(&image)?)?;
            print_header(&img.header);
            println!("code: {} bytes", img.code.len());
            Ok(())
        }
        TasCmd::Image { cmd: ImageCmd::Build { code, output, to_ram } } => {
            let mut img = Image::build(Header::bose_with(DataType::Application), std::fs::read(&code)?)?;
            img.set_header_overwrite(!to_ram);
            std::fs::write(&output, img.to_bytes())?;
            println!("{}: {} bytes (header {HEADER_LEN} + code {})", output.display(), img.to_bytes().len(), img.code.len());
            Ok(())
        }
    }
}

fn read_area(blob: &std::path::Path) -> anyhow::Result<DataArea> {
    let bytes = std::fs::read(blob).with_context(|| format!("reading {}", blob.display()))?;
    DataArea::parse(&bytes).with_context(|| format!("{} is not a valid data-area blob", blob.display()))
}

fn print_header(h: &Header) {
    println!("  VID:PID          {:04x}:{:04x}", h.vendor_id, h.product_id);
    println!("  versions         product {}, firmware {}", h.product_version, h.firmware_version);
    println!("  attributes       0x{:02x} (header overwrite: {})", h.attributes, h.attributes & 0x02 != 0);
    println!("  data type        {:?}", h.data_type);
    println!("  payload          {} bytes", h.payload_size);
    println!("  stock Bose       {}", if h.matches_bose() { "yes (apart from data type / overwrite bit)" } else { "NO" });
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn mode_cmd(mode: Mode, yes: bool) -> anyhow::Result<()> {
    let link = Companion::open()?;
    let current = tas_eeprom::read_block(&link, 0, HEADER_LEN as u8)?;
    let header = Header::parse(&current).context("current EEPROM header is invalid: refusing to touch it")?;
    println!("current header: {}", hex(&current));
    print_header(&header);
    let target = match mode {
        Mode::Show => return Ok(()),
        Mode::Dfu => DataType::Device,
        Mode::App => DataType::Application,
    };
    ensure!(header.matches_bose(), "the header is not the stock Bose header: refusing to modify it");
    ensure!(header.strings.is_empty(), "unexpected USB strings in the header");

    let new = Header::bose_with(target).to_bytes();
    if new == current {
        println!("nothing to do: the header is already in this mode.");
        return Ok(());
    }
    println!("new header:     {}", hex(&new));
    if target == DataType::Device {
        println!(
            "WARNING: after the next power cycle the TAS ROM boots in DFU mode. The speaker no longer works as a \
             speaker (no audio, no HID) until a firmware is downloaded with `oc tas dfu download` (RAM, lost at \
             power-off) or the Bose firmware is restored with `oc tas restore-app` (needs WinUSB on the DFU \
             interface on Windows). The Bose code itself stays untouched in the EEPROM."
        );
    }
    if !yes {
        println!("dry run: add --yes to write the EEPROM.");
        return Ok(());
    }
    tas_eeprom::write_block(&link, 0, &new)?;
    let back = tas_eeprom::read_block(&link, 0, HEADER_LEN as u8)?;
    ensure!(back == new, "read-back mismatch: {} (restore with `oc tas mode app --yes` while the Bose firmware runs)", hex(&back));
    println!("written and verified. Takes effect at the next power cycle of the speaker.");
    Ok(())
}

fn dfu_list() -> anyhow::Result<()> {
    let found = oc_dfu::find(None)?;
    if found.is_empty() {
        println!("no DFU device (in DFU mode the TAS enumerates as 05a7:1020 or ffff:fffe with a DFU interface)");
    }
    for (_, f) in found {
        println!("{:04x}:{:04x} DFU interface {}", f.vendor_id, f.product_id, f.interface);
    }
    Ok(())
}

fn dfu_download(img: &Image, id: Option<(u16, u16)>, yes: bool) -> anyhow::Result<()> {
    let bytes = img.to_bytes();
    let to_eeprom = img.header.attributes & oc_tas_image::ATTR_HEADER_OVERWRITE != 0;
    println!(
        "image: {} bytes (code {}), VID:PID {:04x}:{:04x}, data type {:?}, header written to EEPROM: {}",
        bytes.len(),
        img.code.len(),
        img.header.vendor_id,
        img.header.product_id,
        img.header.data_type,
        to_eeprom
    );
    ensure!(img.code.len() <= oc_tas_image::MAX_CODE, "code too large for the TAS1020B RAM");
    let mut found = oc_dfu::find(id)?;
    if found.is_empty() {
        bail!("no DFU device found: is the TAS in DFU mode, and does its DFU interface use WinUSB (Zadig)?");
    }
    ensure!(found.len() == 1, "several DFU devices found: select one with --id VID:PID");
    let (info, f) = found.remove(0);
    println!("target: {:04x}:{:04x}, interface {}", f.vendor_id, f.product_id, f.interface);
    if !yes {
        println!("dry run: add --yes to download.");
        return Ok(());
    }
    let dev = oc_dfu::DfuDevice::open(&info, &f)?;
    println!("transfer size: {} bytes", dev.transfer_size);
    dev.download(&bytes, |done, total| {
        if done == total || done % 1024 < dev.transfer_size {
            println!("  {done}/{total}");
        }
    })?;
    println!("download complete, USB reset sent: the TAS should now run the downloaded firmware.");
    Ok(())
}

fn restore_app(path: &PathBuf, yes: bool) -> anyhow::Result<()> {
    let dump = std::fs::read(path).with_context(|| format!("reading the Bose dump {}", path.display()))?;
    let mut img = Image::parse(&dump)?;
    ensure!(img.header.matches_bose(), "{} is not the stock Bose image", path.display());
    ensure!(dump[..HEADER_LEN] == BOSE_HEADER, "{}: header differs from the stock dump", path.display());
    img.header.data_type = DataType::Application;
    // Header overwrite on: the ROM writes this (DataType 1) header into the EEPROM, the code goes to RAM and
    // runs right away; the Bose code already in the EEPROM is used again from the next power-up.
    img.set_header_overwrite(true);
    println!(
        "Bose image: code {} bytes. The EEPROM header will be rewritten with DataType 1 and attributes 0x{:02x}; \
         afterwards run `oc tas mode app --yes` (Bose firmware running) to restore the exact stock header (0x08).",
        img.code.len(),
        img.header.attributes
    );
    dfu_download(&img, None, yes)
}
