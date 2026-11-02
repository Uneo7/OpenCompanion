use object::{Object, ObjectSegment};

use crate::format::Segment;
use crate::{Error, Result};

/// Segments to load, regions to zero (the .bss part of segments) and the entry point.
pub fn segments_from_elf(data: &[u8]) -> Result<(Vec<Segment>, Vec<(u32, u32)>, u32)> {
    let file = object::File::parse(data).map_err(|e| Error::Elf(e.to_string()))?;
    let mut segments = Vec::new();
    let mut zero_fills = Vec::new();
    for seg in file.segments() {
        let addr = seg.address() as u32;
        let mem_size = seg.size() as u32;
        let bytes = seg.data().map_err(|e| Error::Elf(e.to_string()))?;
        if !bytes.is_empty() {
            segments.push(Segment { addr, data: bytes.to_vec() });
        }
        let file_size = bytes.len() as u32;
        if mem_size > file_size {
            zero_fills.push((addr + file_size, mem_size - file_size));
        }
    }
    if segments.is_empty() {
        return Err(Error::Elf("no loadable segment".into()));
    }
    Ok((segments, zero_fills, file.entry() as u32))
}
