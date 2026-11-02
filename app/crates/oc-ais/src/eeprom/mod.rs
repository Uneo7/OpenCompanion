//! DSP side: `firmware/dsp/boot` (boot0 trampoline, boot1, updater). Specification: `docs/LAYOUT.md` there.

pub mod body;
pub mod boot;
pub mod graph;
pub mod header;
pub mod image;
pub mod layout;
pub mod lz4;
pub mod update;
pub mod verify;
