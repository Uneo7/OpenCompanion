use eframe::egui::Color32;

/// Premultiplied sRGB colour in 0–1, so that themes interpolate like CSS transitions do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba(pub [f32; 4]);

impl Rgba {
    pub const fn hex(v: u32) -> Self {
        Self::srgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 1.0)
    }

    pub const fn srgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self([r as f32 / 255.0 * a, g as f32 / 255.0 * a, b as f32 / 255.0 * a, a])
    }

    pub fn lerp(self, o: Self, t: f32) -> Self {
        Self(std::array::from_fn(|i| self.0[i] + (o.0[i] - self.0[i]) * t))
    }

    pub fn color(self) -> Color32 {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgba_premultiplied(c(self.0[0]), c(self.0[1]), c(self.0[2]), c(self.0[3]))
    }
}

/// How a token mixes the profile colour `--a1` (`color-mix(in srgb, a1 x%, <with>)`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mix {
    pub a1: f32,
    pub with: Rgba,
}

const fn tint(a1: f32) -> Mix {
    Mix { a1, with: Rgba([0.0; 4]) }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Rgba,
    pub surface: Rgba,
    pub surface_solid: Rgba,
    pub pop: Rgba,
    pub raised: Rgba,
    pub hover: Rgba,
    pub line: Rgba,
    pub line_strong: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub faint: Rgba,
    pub ok: Rgba,
    pub warn: Rgba,
    pub crit: Rgba,
    pub glow: [f32; 3],
    pub lines: f32,
    pub grid: [Rgba; 3],
    pub outline: Rgba,

    pub fill_hover: Mix,
    pub fill_active: Mix,
    pub on_fill: Rgba,
    pub field: Mix,
    pub field_hover: Mix,
    pub field_active: Mix,
    /// Selected segment / chip: a tint in dark, plain white in light.
    pub field_on: Mix,
    pub track: Mix,
    pub thumb_shadow: Rgba,
    pub danger_fill: Rgba,
    pub danger_hover: Rgba,
    pub on_danger: Rgba,
    pub seg_on_shadow: Rgba,
    pub pop_shadow: Rgba,
    pub dialog_shadow: Rgba,
    pub scrim: Rgba,
    pub toast_ok: Rgba,
    pub disabled: f32,
    pub sep: Rgba,
    pub pop_edge: Rgba,
    pub glow_ring: f32,
    /// EQ curve colours are darkened towards the text colour in light (`mixHex(a1, '#1b1830', .12)`).
    pub curve_darken: f32,
}

const WHITE: Rgba = Rgba::hex(0xffffff);
const BLACK: Rgba = Rgba::hex(0x000000);
const INK: Rgba = Rgba::hex(0x1b1830);

pub const DARK: Palette = Palette {
    bg: Rgba::hex(0x111018),
    surface: Rgba::srgba(30, 28, 40, 0.78),
    surface_solid: Rgba::hex(0x1e1c28),
    pop: Rgba::hex(0x24212f),
    raised: Rgba::srgba(255, 255, 255, 0.06),
    hover: Rgba::srgba(255, 255, 255, 0.10),
    line: Rgba::srgba(255, 255, 255, 0.07),
    line_strong: Rgba::srgba(255, 255, 255, 0.13),
    text: Rgba::hex(0xf1effa),
    muted: Rgba::hex(0xa7a2bb),
    faint: Rgba::hex(0x8c87a3),
    ok: Rgba::hex(0x5fc48c),
    warn: Rgba::hex(0xe6b450),
    crit: Rgba::hex(0xf0705f),
    glow: [0.33, 0.27, 0.0],
    lines: 0.22,
    grid: [Rgba::srgba(255, 255, 255, 0.03), Rgba::srgba(255, 255, 255, 0.09), Rgba::srgba(255, 255, 255, 0.22)],
    outline: Rgba::srgba(255, 255, 255, 0.08),

    fill_hover: Mix { a1: 0.86, with: WHITE },
    fill_active: Mix { a1: 0.84, with: BLACK },
    on_fill: Rgba::hex(0x111018),
    field: tint(0.16),
    field_hover: tint(0.24),
    field_active: tint(0.32),
    field_on: tint(0.38),
    track: tint(0.34),
    thumb_shadow: Rgba::srgba(0, 0, 0, 0.45),
    danger_fill: Rgba::hex(0xe0604f),
    danger_hover: Rgba::hex(0xc55444),
    on_danger: WHITE,
    seg_on_shadow: Rgba::srgba(0, 0, 0, 0.25),
    pop_shadow: Rgba::srgba(0, 0, 0, 0.35),
    dialog_shadow: Rgba::srgba(0, 0, 0, 0.40),
    scrim: Rgba::srgba(8, 8, 14, 0.50),
    toast_ok: Rgba::hex(0x5fc48c),
    disabled: 0.45,
    sep: Rgba::srgba(255, 255, 255, 0.08),
    pop_edge: Rgba::srgba(255, 255, 255, 0.09),
    glow_ring: 0.24,
    curve_darken: 0.0,
};

pub const LIGHT: Palette = Palette {
    bg: Rgba::hex(0xf3f1f9),
    surface: Rgba::srgba(255, 255, 255, 0.74),
    surface_solid: Rgba::hex(0xffffff),
    pop: Rgba::hex(0xffffff),
    raised: Rgba::srgba(30, 20, 80, 0.05),
    hover: Rgba::srgba(30, 20, 80, 0.09),
    line: Rgba::srgba(30, 20, 80, 0.08),
    line_strong: Rgba::srgba(30, 20, 80, 0.14),
    text: Rgba::hex(0x1b1830),
    muted: Rgba::hex(0x5f5a75),
    faint: Rgba::hex(0x6a6582),
    ok: Rgba::hex(0x1f9d5c),
    warn: Rgba::hex(0xb57a0a),
    crit: Rgba::hex(0xd8433a),
    glow: [0.46, 0.40, 0.22],
    lines: 0.30,
    grid: [Rgba::srgba(30, 20, 80, 0.045), Rgba::srgba(30, 20, 80, 0.12), Rgba::srgba(30, 20, 80, 0.25)],
    outline: Rgba::srgba(255, 255, 255, 0.08),

    fill_hover: Mix { a1: 0.88, with: INK },
    fill_active: Mix { a1: 0.78, with: INK },
    on_fill: WHITE,
    field: tint(0.14),
    field_hover: tint(0.20),
    field_active: tint(0.28),
    field_on: Mix { a1: 0.0, with: WHITE },
    track: tint(0.32),
    thumb_shadow: Rgba::srgba(30, 20, 80, 0.35),
    danger_fill: Rgba::hex(0xc8372d),
    danger_hover: Rgba::hex(0xb03027),
    on_danger: WHITE,
    seg_on_shadow: Rgba::srgba(30, 20, 80, 0.18),
    pop_shadow: Rgba::srgba(40, 20, 90, 0.14),
    dialog_shadow: Rgba::srgba(40, 20, 90, 0.18),
    scrim: Rgba::srgba(40, 30, 80, 0.25),
    toast_ok: Rgba::hex(0x7fe0a8),
    disabled: 0.5,
    sep: Rgba::srgba(30, 20, 80, 0.10),
    pop_edge: Rgba::srgba(30, 20, 80, 0.10),
    glow_ring: 0.22,
    curve_darken: 0.12,
};

impl Mix {
    fn lerp(self, o: Self, t: f32) -> Self {
        Self { a1: self.a1 + (o.a1 - self.a1) * t, with: self.with.lerp(o.with, t) }
    }

    pub fn with_a1(self, a1: Color32) -> Color32 {
        let c = a1.to_array();
        let a = Rgba([f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0, 1.0]);
        // color-mix(in srgb, a1 p, with): premultiplied interpolation.
        self.with.lerp(a, self.a1).color()
    }
}

impl Palette {
    pub fn lerp(&self, o: &Palette, t: f32) -> Palette {
        let f = |a: f32, b: f32| a + (b - a) * t;
        Palette {
            bg: self.bg.lerp(o.bg, t),
            surface: self.surface.lerp(o.surface, t),
            surface_solid: self.surface_solid.lerp(o.surface_solid, t),
            pop: self.pop.lerp(o.pop, t),
            raised: self.raised.lerp(o.raised, t),
            hover: self.hover.lerp(o.hover, t),
            line: self.line.lerp(o.line, t),
            line_strong: self.line_strong.lerp(o.line_strong, t),
            text: self.text.lerp(o.text, t),
            muted: self.muted.lerp(o.muted, t),
            faint: self.faint.lerp(o.faint, t),
            ok: self.ok.lerp(o.ok, t),
            warn: self.warn.lerp(o.warn, t),
            crit: self.crit.lerp(o.crit, t),
            glow: std::array::from_fn(|i| f(self.glow[i], o.glow[i])),
            lines: f(self.lines, o.lines),
            grid: std::array::from_fn(|i| self.grid[i].lerp(o.grid[i], t)),
            outline: self.outline.lerp(o.outline, t),
            fill_hover: self.fill_hover.lerp(o.fill_hover, t),
            fill_active: self.fill_active.lerp(o.fill_active, t),
            on_fill: self.on_fill.lerp(o.on_fill, t),
            field: self.field.lerp(o.field, t),
            field_hover: self.field_hover.lerp(o.field_hover, t),
            field_active: self.field_active.lerp(o.field_active, t),
            field_on: self.field_on.lerp(o.field_on, t),
            track: self.track.lerp(o.track, t),
            thumb_shadow: self.thumb_shadow.lerp(o.thumb_shadow, t),
            danger_fill: self.danger_fill.lerp(o.danger_fill, t),
            danger_hover: self.danger_hover.lerp(o.danger_hover, t),
            on_danger: self.on_danger.lerp(o.on_danger, t),
            seg_on_shadow: self.seg_on_shadow.lerp(o.seg_on_shadow, t),
            pop_shadow: self.pop_shadow.lerp(o.pop_shadow, t),
            dialog_shadow: self.dialog_shadow.lerp(o.dialog_shadow, t),
            scrim: self.scrim.lerp(o.scrim, t),
            toast_ok: self.toast_ok.lerp(o.toast_ok, t),
            disabled: f(self.disabled, o.disabled),
            sep: self.sep.lerp(o.sep, t),
            pop_edge: self.pop_edge.lerp(o.pop_edge, t),
            glow_ring: f(self.glow_ring, o.glow_ring),
            curve_darken: f(self.curve_darken, o.curve_darken),
        }
    }
}
