//! `--shots` board of the EQ band colours: one row per profile colour of the palette (its two colours, then the
//! 12 band colours of that profile), on the dark and on the light background (`band-palettes.png`,
//! `band-palettes-clair.png`). Drawn from the theme tokens themselves, without the window.

use std::path::Path;

use eframe::egui::Color32;

use crate::model::HUE_CHOICES;
use crate::theme::{self, Theme};

const ROW: u32 = 56;
const PAD: u32 = 24;
const SWATCH: u32 = 64;
const DOT_R: f32 = 15.0;
const STEP: u32 = 42;

pub fn write(dir: &Path) {
    for (light, name) in [(0.0, "band-palettes.png"), (1.0, "band-palettes-clair.png")] {
        let w = PAD * 2 + SWATCH * 2 + 20 + STEP * 12;
        let h = PAD * 2 + ROW * HUE_CHOICES.len() as u32;
        let bg = Theme::resolve(light, Color32::WHITE, Color32::WHITE).bg;
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba(bg.to_array()));
        for (row, hue) in HUE_CHOICES.iter().enumerate() {
            let (a1, a2) = (theme::rgb(hue.a), theme::rgb(hue.b));
            let t = Theme::resolve(light, a1, a2);
            let top = PAD + row as u32 * ROW + (ROW - 32) / 2;
            rect(&mut img, PAD, top, SWATCH, 32, a1);
            rect(&mut img, PAD + SWATCH, top, SWATCH, 32, a2);
            for i in 0..12 {
                let cx = (PAD + SWATCH * 2 + 20 + STEP * i as u32 + STEP / 2) as f32;
                let cy = (top + 16) as f32;
                dot(&mut img, cx, cy, DOT_R, t.band(i));
            }
        }
        let path = dir.join(name);
        if let Err(e) = img.save(&path) {
            eprintln!("board {}: {e}", path.display());
        }
    }
}

fn rect(img: &mut image::RgbaImage, x: u32, y: u32, w: u32, h: u32, c: Color32) {
    for j in y..y + h {
        for i in x..x + w {
            img.put_pixel(i, j, image::Rgba(c.to_array()));
        }
    }
}

/// Anti-aliased disc (coverage from the distance to the edge).
fn dot(img: &mut image::RgbaImage, cx: f32, cy: f32, r: f32, c: Color32) {
    let (x0, x1) = ((cx - r - 1.0) as u32, (cx + r + 1.0) as u32);
    let (y0, y1) = ((cy - r - 1.0) as u32, (cy + r + 1.0) as u32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let k = (r + 0.5 - d).clamp(0.0, 1.0);
            if k > 0.0 {
                let p = img.get_pixel_mut(x, y);
                for ch in 0..3 {
                    let v = [c.r(), c.g(), c.b()][ch];
                    p.0[ch] = (f32::from(p.0[ch]) * (1.0 - k) + f32::from(v) * k).round() as u8;
                }
            }
        }
    }
}
