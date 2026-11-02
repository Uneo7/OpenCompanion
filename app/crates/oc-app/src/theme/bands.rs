//! The 12 EQ band colours, derived from the active profile's colours: a wide analogous range that starts at the
//! profile's first colour and turns towards its second, at one lightness and chroma per theme (OKLCH), so the
//! bands read as one family with the main curve while staying distinct. Neighbouring bands are spread apart on
//! that range (band 1 and 2 are never next to each other).

use eframe::egui::Color32;

/// Range of hues covered by the 12 bands (degrees), from the profile's first colour.
const SPAN: f32 = 285.0;
/// Order of the bands on the range: neighbours far apart, band 1 on the profile colour.
const ORDER: [usize; 12] = [0, 6, 3, 9, 1, 7, 4, 10, 2, 8, 5, 11];
/// (lightness, chroma) on a dark background and on a light one.
const DARK: (f32, f32) = (0.77, 0.16);
const LIGHT: (f32, f32) = (0.6, 0.17);

/// The 12 band colours for the profile colours `a1` → `a2`, on a dark (`light` = false) or light background.
pub fn palette(a1: Color32, a2: Color32, light: bool) -> [Color32; 12] {
    let h1 = hue(a1);
    let d = (hue(a2) - h1 + 540.0) % 360.0 - 180.0;
    let dir = if d < 0.0 { -1.0 } else { 1.0 };
    let (l, c) = if light { LIGHT } else { DARK };
    std::array::from_fn(|i| {
        let k = ORDER[i] as f32 / 11.0;
        from_oklch(l, c, h1 + dir * SPAN * k)
    })
}

fn hue(c: Color32) -> f32 {
    let (_, a, b) = oklab(c);
    b.atan2(a).to_degrees().rem_euclid(360.0)
}

fn lin(u: u8) -> f32 {
    let x = f32::from(u) / 255.0;
    if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
}

fn gamma(x: f32) -> f32 {
    if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 }
}

pub(crate) fn oklab(c: Color32) -> (f32, f32, f32) {
    let (r, g, b) = (lin(c.r()), lin(c.g()), lin(c.b()));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    )
}

fn linear_rgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ]
}

/// OKLCH to sRGB; the chroma is lowered until the colour fits (hue and lightness kept).
fn from_oklch(l: f32, c: f32, h: f32) -> Color32 {
    let (s, co) = h.to_radians().sin_cos();
    let mut c = c;
    loop {
        let rgb = linear_rgb(l, c * co, c * s);
        if rgb.iter().all(|v| (-0.000_5..=1.000_5).contains(v)) || c <= 0.0 {
            let [r, g, b] = rgb.map(|v| (gamma(v.clamp(0.0, 1.0)) * 255.0).round() as u8);
            return Color32::from_rgb(r, g, b);
        }
        c -= 0.005;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HUE_CHOICES;

    fn dist(a: Color32, b: Color32) -> f32 {
        let (l1, a1, b1) = oklab(a);
        let (l2, a2, b2) = oklab(b);
        ((l1 - l2).powi(2) + (a1 - a2).powi(2) + (b1 - b2).powi(2)).sqrt()
    }

    #[test]
    fn every_profile_gets_twelve_distinct_vivid_colours() {
        for h in HUE_CHOICES {
            let (a1, a2) = (crate::theme::rgb(h.a), crate::theme::rgb(h.b));
            for light in [false, true] {
                let p = palette(a1, a2, light);
                for i in 0..12 {
                    let (l, a, b) = oklab(p[i]);
                    assert!((a * a + b * b).sqrt() > 0.09, "vivid: {:?} band {i}", p[i]);
                    let want = if light { LIGHT.0 } else { DARK.0 };
                    assert!((l - want).abs() < 0.02, "one lightness: band {i}");
                    for j in 0..i {
                        assert!(dist(p[i], p[j]) > 0.03, "bands {j} and {i} too close ({light})");
                    }
                    if i > 0 {
                        assert!(dist(p[i], p[i - 1]) > 0.12, "neighbours {} and {i} far apart", i - 1);
                    }
                }
            }
        }
    }
}
