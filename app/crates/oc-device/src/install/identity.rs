//! Identity of the firmware files, checked before anything is written (every path: the development tree, a
//! downloaded release, a `.ocfw` picked by the user). A file that is not an OpenCompanion firmware for that chip
//! is refused with [`NOT_OURS`]; a stock Bose image is never accepted here (only the restore writes the stock
//! firmware, from the user's verified backup).

use oc_tas_image::data_area::DataArea;
use oc_tas_image::Image;

use super::sources::elf_version;

/// The refusal, as the installer shows it.
pub const NOT_OURS: &str = "this file is not an OpenCompanion firmware for this chip";

/// `EM_TI_C6000`.
const EM_TI_C6000: u16 = 140;

fn refuse(why: &str) -> String {
    format!("{NOT_OURS} ({why})")
}

/// The DSP firmware ELF: a TI C6000 ELF carrying our version marker (`"D" OC_VERSION "U"`). The rest (the
/// layout symbol `oc_eeprom_layout_v*`, the load segments, the body size against the U7001 map, the `OCI1`
/// header of boot1) is checked when the image is built (`oc_ais::eeprom::image::app_image`). Returns the version.
pub fn check_dsp_elf(elf: &[u8]) -> Result<String, String> {
    if elf.len() < 52 || elf[..4] != *b"\x7fELF" {
        return Err(refuse("not an ELF file"));
    }
    let little = elf[5] == 1;
    let machine = if little { u16::from_le_bytes([elf[18], elf[19]]) } else { u16::from_be_bytes([elf[18], elf[19]]) };
    if machine != EM_TI_C6000 {
        return Err(refuse("not a TI C6000 program"));
    }
    elf_version(elf).ok_or_else(|| refuse("no OpenCompanion DSP version marker"))
}

/// One USB device descriptor in `b`: `(VID, PID, bcdDevice)` of each 18-byte `12 01` descriptor whose bcdUSB is
/// 1.x or 2.0.
fn device_descriptors(b: &[u8]) -> Vec<(u16, u16, u16)> {
    let le = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    (0..b.len().saturating_sub(17))
        .filter(|&i| b[i] == 0x12 && b[i + 1] == 0x01 && matches!(le(i + 2), 0x0100 | 0x0110 | 0x0200))
        .map(|i| (le(i + 8), le(i + 10), le(i + 12)))
        .collect()
}

fn utf16_contains(b: &[u8], s: &str) -> bool {
    let pat: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    b.windows(pat.len()).any(|w| w == pat.as_slice())
}

/// The USB controller image and its data area: our version marker (`OC-x.y.z`) in the code, the `OCDA` data
/// area (magic and CRC-32 already checked by [`DataArea::parse`]), and our USB identity in its descriptors:
/// 05A7:1020 with bcdDevice ≥ 0x0500 or the product string "OpenCompanion", or the development IDs
/// 1209:0001 / 1209:0002. Returns the version.
pub fn check_tas(img: &Image, data: &DataArea) -> Result<String, String> {
    let code = &img.code;
    let at = code.windows(3).position(|w| w == b"OC-").ok_or_else(|| refuse("no OpenCompanion USB version marker"))?;
    let version: String = code[at + 3..].iter().take(8).take_while(|b| b.is_ascii_digit() || **b == b'.').map(|&b| b as char).collect();
    if version.is_empty() {
        return Err(refuse("no OpenCompanion USB version marker"));
    }
    let blob = [data.payload.as_slice(), code].concat();
    let named = utf16_contains(&blob, "OpenCompanion");
    let ours = device_descriptors(&blob).into_iter().any(|(vid, pid, bcd)| match (vid, pid) {
        (0x05A7, 0x1020) => bcd >= 0x0500 || named,
        (0x1209, 0x0001 | 0x0002) => true,
        _ => false,
    });
    if !ours {
        return Err(refuse("no OpenCompanion USB identity in its descriptors"));
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/release");

    /// A stock dump of the speaker (`u6002_tas.bin` / `u7001_dsp.bin`): in `OC_STOCK_DUMPS` when set, else in
    /// the oldest backup folder (`<data>/backups/<speaker>/<stamp>/`: the first backup is the one taken from the
    /// stock firmware). `None` when there is none (CI).
    fn stock_dump(name: &str) -> Option<Vec<u8>> {
        if let Some(d) = std::env::var_os("OC_STOCK_DUMPS") {
            return std::fs::read(std::path::Path::new(&d).join(name)).ok();
        }
        let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(crate::paths::backups_dir())
            .ok()?
            .flatten()
            .filter_map(|speaker| std::fs::read_dir(speaker.path()).ok())
            .flat_map(|stamps| stamps.flatten().map(|e| e.path().join(name)))
            .filter(|p| p.is_file())
            .collect();
        found.sort();
        std::fs::read(found.first()?).ok()
    }

    #[test]
    fn foreign_dsp_files_are_refused() {
        assert!(check_dsp_elf(&[0x5a; 4096]).unwrap_err().starts_with(NOT_OURS), "random bytes");
        assert!(check_dsp_elf(b"\x7fELF").is_err(), "truncated");
        // An ELF of another machine (x86-64 header) is refused.
        let mut other = vec![0u8; 64];
        other[..4].copy_from_slice(b"\x7fELF");
        other[5] = 1;
        other[18] = 62;
        assert!(check_dsp_elf(&other).unwrap_err().contains("TI C6000"));
        // A C6000 ELF without our marker (another project).
        other[18] = 140;
        assert!(check_dsp_elf(&other).unwrap_err().contains("version marker"));
        match stock_dump("u7001_dsp.bin") {
            Some(stock) => assert!(check_dsp_elf(&stock).unwrap_err().starts_with(NOT_OURS), "the stock Bose DSP image"),
            None => eprintln!("skipped: no stock DSP dump (OC_STOCK_DUMPS or a backup)"),
        }
        match std::fs::read(format!("{DEV}/opencompanion_dsp.out")) {
            Ok(ours) => assert!(check_dsp_elf(&ours).is_ok(), "our firmware"),
            Err(_) => eprintln!("skipped: firmware/release not present"),
        }
    }

    #[test]
    fn foreign_usb_files_are_refused() {
        let data = std::fs::read(format!("{DEV}/oc_tas_data.bin"));
        let img = std::fs::read(format!("{DEV}/oc_tas.dfu"));
        let (Ok(data), Ok(img)) = (data, img) else {
            eprintln!("skipped: firmware/release not present");
            return;
        };
        let area = DataArea::parse(&data).unwrap();
        let ours = Image::parse(&img).unwrap();
        assert!(check_tas(&ours, &area).is_ok(), "our firmware");
        match stock_dump("u6002_tas.bin") {
            Some(stock) => {
                // The stock image: no OpenCompanion marker (even with our data area), and no OCDA data area.
                let stock_img = Image::parse(&stock).expect("the stock dump starts with a TI boot header");
                assert!(check_tas(&stock_img, &area).unwrap_err().contains("version marker"), "the stock Bose image");
                assert!(DataArea::parse(&stock[0x1800..]).is_err(), "the stock data area");
            }
            None => eprintln!("skipped: no stock USB controller dump (OC_STOCK_DUMPS or a backup)"),
        }
        // Our version marker, but a data area that names another device: refused.
        let foreign = DataArea::new(area.version, vec![0x12, 0x01, 0x10, 0x01, 0, 0, 0, 64, 0x34, 0x12, 0x78, 0x56, 0, 1, 1, 2, 3, 1]).unwrap();
        let fake = Image::build(ours.header.clone(), b"..OC-0.4.9\0".to_vec()).unwrap();
        assert!(check_tas(&fake, &foreign).unwrap_err().contains("USB identity"));
        assert!(check_tas(&fake, &area).is_ok(), "with our data area");
        assert!(Image::parse(&[0x5a; 64]).is_err(), "random bytes");
        assert!(Image::parse(&img[..10]).is_err(), "truncated");
    }
}
