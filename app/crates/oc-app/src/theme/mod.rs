pub mod bands;
pub mod metrics;
pub mod motion;
pub mod palette;
pub mod typography;

use std::sync::Arc;

use eframe::egui::{Color32, Context, Id};

use palette::{Palette, Rgba, DARK, LIGHT};

#[derive(Debug, Clone)]
pub struct Theme {
    /// 0 = dark, 1 = light (in between while the theme cross-fades).
    pub light: f32,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_solid: Color32,
    pub pop: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub crit: Color32,
    /// The active profile's two colours (--a1, --a2).
    pub a1: Color32,
    pub a2: Color32,
    pub glow: [f32; 3],
    pub lines: f32,
    pub grid: [Color32; 3],
    pub outline: Color32,

    pub fill: Color32,
    pub fill_hover: Color32,
    pub fill_active: Color32,
    pub on_fill: Color32,
    pub field: Color32,
    pub field_hover: Color32,
    pub field_active: Color32,
    pub field_on: Color32,
    pub track: Color32,
    pub thumb: Color32,
    pub thumb_shadow: Color32,
    pub danger_fill: Color32,
    pub danger_hover: Color32,
    pub on_danger: Color32,
    pub seg_on_shadow: Color32,
    pub pop_shadow: Color32,
    pub dialog_shadow: Color32,
    pub scrim: Color32,
    pub toast_ok: Color32,
    pub disabled: f32,
    pub sep: Color32,
    pub pop_edge: Color32,
    pub glow_ring: Color32,
    pub curve1: Color32,
    pub curve2: Color32,
    /// The 12 EQ band colours of the active profile ([`bands::palette`]).
    pub bands: [Color32; 12],
}

impl Theme {
    pub fn resolve(light_t: f32, a1: Color32, a2: Color32) -> Self {
        let p: Palette = DARK.lerp(&LIGHT, light_t);
        let c = |r: Rgba| r.color();
        let ink = Color32::from_rgb(0x1b, 0x18, 0x30);
        Self {
            light: light_t,
            bg: c(p.bg),
            surface: c(p.surface),
            surface_solid: c(p.surface_solid),
            pop: c(p.pop),
            raised: c(p.raised),
            hover: c(p.hover),
            line: c(p.line),
            line_strong: c(p.line_strong),
            text: c(p.text),
            muted: c(p.muted),
            faint: c(p.faint),
            ok: c(p.ok),
            warn: c(p.warn),
            crit: c(p.crit),
            a1,
            a2,
            glow: p.glow,
            lines: p.lines,
            grid: p.grid.map(c),
            outline: c(p.outline),
            fill: a1,
            fill_hover: p.fill_hover.with_a1(a1),
            fill_active: p.fill_active.with_a1(a1),
            on_fill: c(p.on_fill),
            field: p.field.with_a1(a1),
            field_hover: p.field_hover.with_a1(a1),
            field_active: p.field_active.with_a1(a1),
            field_on: p.field_on.with_a1(a1),
            track: p.track.with_a1(a1),
            thumb: Color32::WHITE,
            thumb_shadow: c(p.thumb_shadow),
            danger_fill: c(p.danger_fill),
            danger_hover: c(p.danger_hover),
            on_danger: c(p.on_danger),
            seg_on_shadow: c(p.seg_on_shadow),
            pop_shadow: c(p.pop_shadow),
            dialog_shadow: c(p.dialog_shadow),
            scrim: c(p.scrim),
            toast_ok: c(p.toast_ok),
            disabled: p.disabled,
            sep: c(p.sep),
            pop_edge: c(p.pop_edge),
            glow_ring: alpha(a1, p.glow_ring),
            curve1: mix(a1, ink, p.curve_darken),
            curve2: mix(a2, ink, p.curve_darken),
            bands: {
                let (d, l) = (bands::palette(a1, a2, false), bands::palette(a1, a2, true));
                std::array::from_fn(|i| mix(d[i], l[i], light_t))
            },
        }
    }

    /// Colour of EQ band `i`: from the active profile's palette, for the current theme.
    pub fn band(&self, i: usize) -> Color32 {
        self.bands[i % 12]
    }
}

fn key() -> Id {
    Id::NULL.with("oc-theme")
}

pub fn set(ctx: &Context, theme: Theme) {
    ctx.data_mut(|d| d.insert_temp(key(), Arc::new(theme)));
}

pub fn get(ctx: &Context) -> Arc<Theme> {
    ctx.data(|d| d.get_temp::<Arc<Theme>>(key()))
        .unwrap_or_else(|| Arc::new(Theme::resolve(0.0, Color32::from_rgb(0x7b, 0x6c, 0xff), Color32::from_rgb(0x2f, 0xc4, 0xff))))
}

/// `color-mix(in srgb, c a%, transparent)`: the same colour with its alpha scaled by `a`.
pub fn alpha(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

/// `color-mix(in srgb, a (1 − t), b t)` on premultiplied channels.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let (x, y) = (a.to_array(), b.to_array());
    let ch = |i: usize| (f32::from(x[i]) + (f32::from(y[i]) - f32::from(x[i])) * t).round() as u8;
    Color32::from_rgba_premultiplied(ch(0), ch(1), ch(2), ch(3))
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_and_alpha() {
        let w = Color32::WHITE;
        assert_eq!(alpha(w, 0.5).a(), 128);
        assert_eq!(mix(Color32::BLACK, w, 0.5), Color32::from_gray(128));
        let t = Theme::resolve(1.0, w, w);
        assert_eq!(t.bg, Color32::from_rgb(0xf3, 0xf1, 0xf9));
        let d = Theme::resolve(0.0, w, w);
        assert_eq!(d.text, Color32::from_rgb(0xf1, 0xef, 0xfa));
    }

    #[test]
    fn text_on_fills_follows_the_theme() {
        let a1 = Color32::from_rgb(0x7b, 0x6c, 0xff);
        let d = Theme::resolve(0.0, a1, a1);
        let l = Theme::resolve(1.0, a1, a1);
        assert_eq!(d.on_fill, Color32::from_rgb(0x11, 0x10, 0x18));
        assert_eq!(l.on_fill, Color32::WHITE);
        // Flat fills: the primary fill is the profile colour itself.
        assert_eq!(d.fill, a1);
        assert_eq!(l.field_on, Color32::WHITE);
        assert!(d.field.a() < 60);
    }

    #[test]
    fn band_colours_follow_the_profile_and_the_theme() {
        let (a, b) = (Color32::from_rgb(0xff, 0xb5, 0x47), Color32::from_rgb(0x22, 0xd3, 0xb8));
        let (amber, teal) = (Theme::resolve(0.0, a, a), Theme::resolve(0.0, b, b));
        assert_ne!(amber.band(0), teal.band(0));
        assert_ne!(amber.band(0), Theme::resolve(1.0, a, a).band(0), "separate light tokens");
        assert_eq!(amber.band(12), amber.band(0));
    }
}
