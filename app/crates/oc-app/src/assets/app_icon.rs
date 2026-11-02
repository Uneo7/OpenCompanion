use eframe::egui::IconData;

const PNG: &[u8] = include_bytes!("../../../../packaging/icons/png/opencompanion-256.png");

/// The decoded icon (`None` only if the embedded file is broken: the OS default icon is used then).
pub fn icon_data() -> Option<IconData> {
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?.to_rgba8();
    Some(IconData { width: img.width(), height: img.height(), rgba: img.into_raw() })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_embedded_icon_decodes() {
        let i = super::icon_data().expect("icon");
        assert_eq!((i.width, i.height, i.rgba.len()), (256, 256, 256 * 256 * 4));
    }
}
