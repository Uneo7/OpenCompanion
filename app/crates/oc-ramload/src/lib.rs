//! Sequence (Bose DSP firmware running):
//! 1. check that the loader area 0x10023000..0x10024DFF still holds the stock `.cinit` table (dead after boot),
//!    sampled twice, 1 s apart,
//! 2. write the stub there with BWW and verify it with BRR,
//! 3. hijack the process pointer of the output meter module to the stub entry,
//! 4. talk to the stub through the TAS `E` command: write the image, check CRCs, jump to the entry point.
//!
//! Going back: standby or unplugging makes the DSP reboot the Bose firmware from its EEPROM.

mod image;
mod loader;
mod stub;

pub use image::{read_image, LoadImage};
pub use loader::{load_image, probe, write_chunk};
pub use stub::{check_area_free, hijack, hijack_with, install_stub, stock_ram};

pub const RAM_LO: u32 = 0x1000_0000;
pub const RAM_HI: u32 = 0x1003_0000;
/// Loader area: tail of the stock `.cinit` table, only read by `c_int00` at boot.
pub const LOADER_LO: u32 = 0x1002_3000;
pub const LOADER_HI: u32 = 0x1002_4E00;
/// Output meter (class 0x1002CDDC), last child of the root graph.
pub const HIJACK_MODULE: u32 = 0x1001_1C4C;
/// Process pointer of the hijacked module.
pub const HIJACK_SLOT: u32 = HIJACK_MODULE + 20;
pub const HIJACK_ORIGINAL: u32 = 0x1002_CDDC;
/// Flags word of the hijacked module (must be 0x3: audio module called through its process pointer).
pub const HIJACK_FLAGS: u32 = HIJACK_MODULE + 16;

/// Estimated duration of one transaction (HID round trip + 20 ms TAS delay + I2C frames).
pub const EST_TRANSACTION_S: f64 = 0.045;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Load(String),
    #[error(transparent)]
    Link(#[from] oc_protocol::Error),
    #[error(transparent)]
    Ais(#[from] oc_ais::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn fail<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Load(msg.into()))
}

/// Number of transactions of the whole operation, for the plan and the time estimate.
pub fn plan(stub: &LoadImage, image: Option<&LoadImage>) -> Vec<String> {
    let mut out = Vec::new();
    let n_bww: usize = stub.segments.iter().map(|(_, d)| d.len().div_ceil(20)).sum();
    let n_brr: usize =
        stub.segments.iter().map(|(_, d)| d.len().div_ceil(16)).sum::<usize>() + 2 * ((LOADER_HI - LOADER_LO) as usize).div_ceil(0x100);
    let stub_bytes: usize = stub.segments.iter().map(|(_, d)| d.len()).sum();
    out.push(format!("stub: entry 0x{:08x}, {} segment(s), {stub_bytes} bytes", stub.entry, stub.segments.len()));
    for (a, d) in &stub.segments {
        out.push(format!("  0x{a:08x} +0x{:x}", d.len()));
    }
    out.push(format!("  {n_bww} BWW + {n_brr} BRR transactions"));
    out.push(format!(
        "hijack: module 0x{HIJACK_MODULE:08x}, write 0x{:08x} at 0x{HIJACK_SLOT:08x} (was 0x{HIJACK_ORIGINAL:08x})",
        stub.entry
    ));
    let mut total = n_bww + n_brr;
    if let Some(img) = image {
        let mut n: usize = img
            .segments
            .iter()
            .map(|(_, d)| d.len().div_ceil(loader::WRITE_CHUNK) + d.len().div_ceil(loader::CRC_CHUNK))
            .sum();
        n += img.fills.len() + 1;
        let bytes: usize = img.segments.iter().map(|(_, d)| d.len()).sum();
        out.push(format!(
            "image: entry 0x{:08x}, {bytes} bytes in {} segment(s), {} fill(s), {n} loader transactions",
            img.entry,
            img.segments.len(),
            img.fills.len()
        ));
        for (a, d) in &img.segments {
            out.push(format!("  0x{a:08x} +0x{:x}", d.len()));
        }
        for (a, len, b) in &img.fills {
            out.push(format!("  fill 0x{a:08x} +0x{len:x} = 0x{b:02x}"));
        }
        total += n;
    }
    out.push(format!("estimated time: {:.0} s ({total} transactions)", total as f64 * EST_TRANSACTION_S));
    out
}
