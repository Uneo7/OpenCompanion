//! AIS images (TI SPRAA69) for the ROM bootloader of the DA705 / C672x DSP.
//! Format: magic word `0x41504954`, then little-endian `0x585359xx` commands, 4-byte aligned.
//! Bose's firmware adds opcode 0x09 (zlib sections): supported for reading only.

pub mod eeprom;
mod elf;
mod format;
mod reader;
mod writer;

pub use elf::segments_from_elf;
pub use format::{Command, Image, Segment, MAGIC};
pub use reader::parse;
pub use writer::{build, write, BuildOptions};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing AIS magic word (0x{0:08x})")]
    BadMagic(u32),
    #[error("image truncated at offset 0x{0:x}")]
    Truncated(usize),
    #[error("unknown opcode 0x{opcode:08x} at offset 0x{offset:x}")]
    UnknownOpcode { opcode: u32, offset: usize },
    #[error("inconsistent zlib section at offset 0x{0:x}")]
    BadZlib(usize),
    #[error("invalid ELF: {0}")]
    Elf(String),
    #[error("segment 0x{addr:08x}+0x{len:x} is outside the allowed RAM")]
    OutOfRange { addr: u32, len: usize },
    #[error("EEPROM layout: {0}")]
    Layout(String),
}

pub type Result<T> = std::result::Result<T, Error>;
