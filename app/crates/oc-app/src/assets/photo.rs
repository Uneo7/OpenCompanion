//! WARNING: third-party product photo, taken from the design folder for the prototype only. It must be
//! replaced by the project's own photo (without logos) before any public release; see
//! `oc-app/assets/photo/SOURCES.md`.

use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};

const PNG: &[u8] = include_bytes!("../../assets/photo/c5-system.png");

/// Aspect ratio of the photo's drawing box (the mockup's SVG viewBox, 2018 × 1135).
pub const ASPECT: f32 = 2018.0 / 1135.0;
pub const WIDTH: f32 = 500.0;

pub fn texture(ctx: &Context) -> Option<TextureHandle> {
    let id = eframe::egui::Id::new("c5-photo");
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(t);
    }
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let options = TextureOptions {
        mipmap_mode: Some(eframe::egui::TextureFilter::Linear),
        ..TextureOptions::LINEAR
    };
    let tex = ctx.load_texture("c5-photo", color, options);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}
