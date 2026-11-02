//! Each SVG is rasterised by resvg at the exact pixel size it is drawn at (white, then tinted when painted) and
//! kept as a texture: crisp at every size, in both themes. The stroke is set per look so the icons sit with
//! the typography: Lucide draws 2 px on 24, the app uses 2 (1.75 for the window buttons).

use eframe::egui::{
    emath::Rot2,
    epaint::{Mesh, Shape},
    Color32, ColorImage, Context, Id, Painter, Pos2, Rect, TextureHandle, TextureOptions, Vec2,
};
use resvg::{tiny_skia, usvg};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Look {
    /// The UI's stroke (2 on 24).
    Regular,
    /// Thinner (1.75 on 24): the window buttons.
    Light,
    Fill,
}

impl Look {
    fn stroke(self) -> &'static str {
        match self {
            Look::Regular | Look::Fill => "2",
            Look::Light => "1.75",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon {
    pub name: &'static str,
    svg: &'static str,
    pub look: Look,
}

macro_rules! lucide {
    ($look:ident, $name:literal) => {
        Icon { name: $name, svg: include_str!(concat!("../../assets/icons/lucide/", $name, ".svg")), look: Look::$look }
    };
}

pub const EQ: Icon = lucide!(Regular, "audio-lines");
pub const DOT: Icon = lucide!(Fill, "circle");
pub const CLOSE: Icon = lucide!(Light, "x");
pub const MINIMIZE: Icon = lucide!(Light, "minus");

pub const USB: Icon = lucide!(Regular, "usb");
pub const VOLUME_ON: Icon = lucide!(Regular, "volume-2");
pub const VOLUME_OFF: Icon = lucide!(Regular, "volume-x");
pub const CHECK_CIRCLE: Icon = lucide!(Regular, "circle-check");
pub const CHECK: Icon = lucide!(Regular, "check");
pub const CHEVRON_DOWN: Icon = lucide!(Regular, "chevron-down");
pub const MORE: Icon = lucide!(Regular, "ellipsis");
pub const PLUS: Icon = lucide!(Regular, "plus");
pub const UPLOAD: Icon = lucide!(Regular, "upload");
pub const PENCIL: Icon = lucide!(Regular, "pencil");
pub const TRASH: Icon = lucide!(Regular, "trash-2");
pub const EAR: Icon = lucide!(Regular, "ear");
pub const SPEAKER: Icon = lucide!(Regular, "volume-2");
pub const SPEAKERS: Icon = lucide!(Regular, "speaker");
pub const SUB: Icon = lucide!(Regular, "volume-1");
pub const SLIDERS: Icon = lucide!(Regular, "sliders-horizontal");
pub const HEADPHONES: Icon = lucide!(Regular, "headphones");
pub const CHIP: Icon = lucide!(Regular, "cpu");
pub const TERMINAL: Icon = lucide!(Regular, "terminal");
pub const ROTATE: Icon = lucide!(Regular, "rotate-ccw");
pub const LOCK: Icon = lucide!(Regular, "lock");
pub const MOON: Icon = lucide!(Regular, "moon");
pub const SUN: Icon = lucide!(Regular, "sun");
pub const INFO: Icon = lucide!(Regular, "info");
pub const DRIVE: Icon = lucide!(Regular, "hard-drive");
pub const ENTER: Icon = lucide!(Regular, "corner-down-left");
pub const BYPASS: Icon = lucide!(Regular, "power");
pub const PULSE: Icon = lucide!(Regular, "activity");
pub const THERMO: Icon = lucide!(Regular, "thermometer");
pub const BOLT: Icon = lucide!(Regular, "zap");
pub const WAVE: Icon = lucide!(Regular, "audio-waveform");
pub const EXPORT: Icon = lucide!(Regular, "share");
pub const IMPORT: Icon = lucide!(Regular, "download");
pub const PLAY: Icon = lucide!(Regular, "play");
pub const SPARKLE: Icon = lucide!(Regular, "sparkles");
pub const MONITOR: Icon = lucide!(Regular, "monitor");
pub const AUTO: Icon = lucide!(Regular, "contrast");
pub const WARN: Icon = lucide!(Regular, "circle-alert");
pub const HELP: Icon = lucide!(Regular, "circle-question-mark");
pub const BACK: Icon = lucide!(Regular, "arrow-left");
pub const COPY: Icon = lucide!(Regular, "copy");
pub const UPDATE: Icon = lucide!(Regular, "circle-arrow-up");
pub const PLUG: Icon = lucide!(Regular, "plug");
pub const UNPLUG: Icon = lucide!(Regular, "unplug");

impl Icon {
    pub fn paint(self, painter: &Painter, center: Pos2, size: f32, color: Color32) {
        self.paint_rotated(painter, center, size, color, 0.0);
    }

    pub fn paint_rotated(self, painter: &Painter, center: Pos2, size: f32, color: Color32, angle: f32) {
        let ctx = painter.ctx();
        let ppp = ctx.pixels_per_point();
        let px = (size * ppp).round().max(1.0);
        // On the pixel grid: one texel per physical pixel.
        let min = ((center.to_vec2() * ppp - Vec2::splat(px / 2.0)).round() / ppp).to_pos2();
        let rect = Rect::from_min_size(min, Vec2::splat(px / ppp));
        let Some(tex) = texture(ctx, self, px as u32) else { return };
        let mut mesh = Mesh::with_texture(tex.id());
        mesh.add_rect_with_uv(rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), color);
        if angle != 0.0 {
            mesh.rotate(Rot2::from_angle(angle), rect.center());
        }
        painter.add(Shape::mesh(mesh));
    }
}

/// The icon rasterised at `px` × `px` (white on transparent), made once per size and kept.
fn texture(ctx: &Context, icon: Icon, px: u32) -> Option<TextureHandle> {
    let id = Id::new(("lucide", icon.name, icon.look, px));
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(t);
    }
    let pixels = rasterise(icon, px)?;
    let tex = ctx.load_texture(format!("lucide-{}-{px}", icon.name), pixels, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

fn rasterise(icon: Icon, px: u32) -> Option<ColorImage> {
    let mut svg = icon.svg.replace("currentColor", "#ffffff").replace("stroke-width=\"2\"", &format!("stroke-width=\"{}\"", icon.look.stroke()));
    if icon.look == Look::Fill {
        svg = svg.replace("fill=\"none\"", "fill=\"#ffffff\"");
    }
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    let k = px as f32 / tree.size().width();
    resvg::render(&tree, tiny_skia::Transform::from_scale(k, k), &mut pixmap.as_mut());
    Some(ColorImage::from_rgba_premultiplied([px as usize, px as usize], pixmap.data()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_renders() {
        let all = [
            EQ, DOT, CLOSE, MINIMIZE, USB, VOLUME_ON, VOLUME_OFF, CHECK_CIRCLE, CHECK, CHEVRON_DOWN, MORE, PLUS, UPLOAD,
            PENCIL, TRASH, EAR, SPEAKER, SPEAKERS, SUB, SLIDERS, HEADPHONES, CHIP, TERMINAL, ROTATE, LOCK, MOON, SUN,
            INFO, DRIVE, ENTER, BYPASS, PULSE, THERMO, BOLT, WAVE, EXPORT, IMPORT, PLAY, SPARKLE, MONITOR, AUTO, WARN,
            HELP, BACK, COPY, UPDATE, PLUG, UNPLUG,
        ];
        for i in all {
            let img = rasterise(i, 18).unwrap_or_else(|| panic!("{}", i.name));
            assert!(img.pixels.iter().any(|p| p.a() > 0), "{} is blank", i.name);
        }
    }
}
