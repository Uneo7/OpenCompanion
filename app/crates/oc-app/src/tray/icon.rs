//! The tray icon image: 64 px, sharp on HiDPI notification areas and the Retina menu bar (the OS scales it).

const PNG: &[u8] = include_bytes!("../../../../packaging/icons/png/opencompanion-64.png");

/// RGBA pixels, width, height.
pub fn rgba() -> Result<(Vec<u8>, u32, u32), String> {
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = img.dimensions();
    Ok((img.into_raw(), w, h))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tray_icon_decodes() {
        let (px, w, h) = super::rgba().expect("icon");
        assert_eq!(px.len(), (w * h * 4) as usize);
    }
}
