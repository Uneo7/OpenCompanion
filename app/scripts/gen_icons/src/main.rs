//! Deterministic: same SVG, same bytes. Usage: `cargo run --release --manifest-path scripts/gen_icons/Cargo.toml`
//! from `app/` (or `scripts/gen_icons.ps1` / `.sh`).

use std::fs;
use std::path::{Path, PathBuf};

use resvg::{tiny_skia, usvg};

const PNG_SIZES: [u32; 10] = [16, 24, 32, 48, 64, 128, 256, 512, 1024, 22];
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
/// (ICNS type, pixel size).
const ICNS: [(&[u8; 4], u32); 11] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"icp6", 64),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic13", 256),
    (b"ic14", 512),
];
const HICOLOR: [u32; 9] = [16, 22, 24, 32, 48, 64, 128, 256, 512];

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn render(tree: &usvg::Tree, size: u32) -> Result<Vec<u8>> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or("bad size")?;
    let k = size as f32 / tree.size().width();
    resvg::render(tree, tiny_skia::Transform::from_scale(k, k), &mut pixmap.as_mut());
    let rgba: Vec<u8> = pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect();
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, size, size);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Best);
        enc.write_header()?.write_image_data(&rgba)?;
    }
    Ok(out)
}

/// ICO with PNG entries (Windows Vista and later).
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 0, 1, 0];
    out.extend((images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, data) in images {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([dim, dim, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend(data);
    }
    out
}

fn icns(entries: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (kind, data) in entries {
        body.extend(*kind);
        body.extend((8 + data.len() as u32).to_be_bytes());
        body.extend(data);
    }
    let mut out = b"icns".to_vec();
    out.extend((8 + body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

fn write(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, data)?;
    println!("{} ({} bytes)", path.display(), data.len());
    Ok(())
}

fn main() -> Result<()> {
    let app = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    let svg = fs::read_to_string(app.join("packaging/icons/opencompanion.svg"))?;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())?;
    let pkg = app.join("packaging");

    write(&pkg.join("linux/hicolor/scalable/apps/opencompanion.svg"), svg.as_bytes())?;
    let mut sizes: Vec<u32> = PNG_SIZES.to_vec();
    sizes.sort_unstable();
    let png = |n: u32| render(&tree, n);
    for n in &sizes {
        write(&pkg.join(format!("icons/png/opencompanion-{n}.png")), &png(*n)?)?;
    }
    let ico_images = ICO_SIZES.iter().map(|n| Ok((*n, png(*n)?))).collect::<Result<Vec<_>>>()?;
    write(&pkg.join("windows/opencompanion.ico"), &ico(&ico_images))?;
    let icns_images = ICNS.iter().map(|(k, n)| Ok((*k, png(*n)?))).collect::<Result<Vec<_>>>()?;
    write(&pkg.join("macos/opencompanion.icns"), &icns(&icns_images))?;
    for n in HICOLOR {
        write(&pkg.join(format!("linux/hicolor/{n}x{n}/apps/opencompanion.png")), &png(n)?)?;
    }
    Ok(())
}
