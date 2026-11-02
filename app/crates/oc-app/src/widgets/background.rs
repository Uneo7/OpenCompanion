use eframe::egui::{
    epaint::{Mesh, PathStroke, Vertex, WHITE_UV},
    Color32, Painter, Pos2, Rect, Shape, Vec2,
};

use crate::theme::Theme;

/// Height of the backdrop's own coordinate space (the whole window).
pub const LINES_H: f32 = 720.0;
/// Fade centre when no photo is shown (fraction of the window).
pub const FADE_DEFAULT: Vec2 = Vec2::new(0.5, 0.48);

struct Glow {
    at: [f32; 2],
    radii: [f32; 2],
    second: bool,
}

const GLOWS: [Glow; 3] = [
    Glow { at: [0.04, -0.04], radii: [560.0, 400.0], second: false },
    Glow { at: [1.0, 1.06], radii: [660.0, 460.0], second: true },
    Glow { at: [0.72, 0.06], radii: [460.0, 320.0], second: true },
];

fn to_f(c: Color32) -> [f32; 3] {
    [f32::from(c.r()) / 255.0, f32::from(c.g()) / 255.0, f32::from(c.b()) / 255.0]
}

fn window_color(p: Pos2, size: Vec2, t: &Theme) -> Color32 {
    let mut out = to_f(t.bg);
    let (a1, a2) = (to_f(t.a1), to_f(t.a2));
    // CSS paints the first layer on top: composite from the last one up.
    for (i, g) in GLOWS.iter().enumerate().rev() {
        let c = Vec2::new(g.at[0] * size.x, g.at[1] * size.y);
        let d = (((p.x - c.x) / g.radii[0]).powi(2) + ((p.y - c.y) / g.radii[1]).powi(2)).sqrt();
        let a = t.glow[i] * (1.0 - d / 0.7).max(0.0);
        if a > 0.0 {
            let col = if g.second { a2 } else { a1 };
            for k in 0..3 {
                out[k] = out[k] * (1.0 - a) + col[k] * a;
            }
        }
    }
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(out[0]), b(out[1]), b(out[2]))
}

pub fn paint_window(painter: &Painter, win: Rect, radius: f32, t: &Theme) {
    let size = win.size();
    let r = radius;
    let mut xs = vec![0.0, r];
    let mut x = r + 20.0;
    while x < size.x - r {
        xs.push(x);
        x += 20.0;
    }
    xs.extend([size.x - r, size.x]);
    let mut ys = vec![0.0, r];
    let mut y = r + 20.0;
    while y < size.y - r {
        ys.push(y);
        y += 20.0;
    }
    ys.extend([size.y - r, size.y]);
    xs.dedup();
    ys.dedup();

    let mut mesh = Mesh::default();
    let color = |p: Pos2| window_color(p, size, t);
    let at = |p: Pos2| win.min + p.to_vec2();
    let nx = xs.len();
    for &yy in &ys {
        for &xx in &xs {
            let p = Pos2::new(xx, yy);
            mesh.vertices.push(Vertex { pos: at(p), uv: WHITE_UV, color: color(p) });
        }
    }
    let corner = |x0: f32, y0: f32| r > 0.0 && (x0 < r || x0 >= size.x - r) && (y0 < r || y0 >= size.y - r);
    for (j, &y0) in ys[..ys.len() - 1].iter().enumerate() {
        for (i, &x0) in xs[..nx - 1].iter().enumerate() {
            if corner(x0, y0) {
                continue;
            }
            let a = (j * nx + i) as u32;
            let b = a + 1;
            let c = a + nx as u32;
            let d = c + 1;
            mesh.add_triangle(a, b, d);
            mesh.add_triangle(a, d, c);
        }
    }
    if r > 0.0 {
        for (cx, cy, start) in [(r, r, 180.0f32), (size.x - r, r, 270.0), (size.x - r, size.y - r, 0.0), (r, size.y - r, 90.0)] {
            let c = Pos2::new(cx, cy);
            let base = mesh.vertices.len() as u32;
            mesh.vertices.push(Vertex { pos: at(c), uv: WHITE_UV, color: color(c) });
            let n = 8;
            for k in 0..=n {
                let ang = (start + 90.0 * k as f32 / n as f32).to_radians();
                let p = c + Vec2::new(ang.cos(), ang.sin()) * r;
                mesh.vertices.push(Vertex { pos: at(p), uv: WHITE_UV, color: color(p) });
            }
            for k in 0..n {
                mesh.add_triangle(base, base + 1 + k, base + 2 + k);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}

fn contour(i: usize) -> Vec<Pos2> {
    let fi = i as f32;
    let base = -40.0 + fi * 46.0;
    let amp = 26.0 + (i % 5) as f32 * 7.0;
    let ph = fi * 0.55;
    (0..=57)
        .map(|k| {
            let x = -20.0 + k as f32 * 20.0;
            let y = base + (x / 210.0 + ph).sin() * amp + (x / 83.0 + ph * 1.7).sin() * amp * 0.22;
            Pos2::new(x, (y * 10.0).round() / 10.0)
        })
        .collect()
}

pub fn window_of(main: Rect) -> Rect {
    Rect::from_min_max(Pos2::new(main.left(), main.top() - crate::theme::metrics::TOOLBAR_H), main.max)
}

/// Opacity of the lines' mask at window-space point `l` (1100 × 720): the SVG's radial gradient
/// (`r = .38`, black up to 58 % of the radius, then up to white at the edge), centred on `fade`.
fn mask(l: Pos2, fade: Vec2) -> f32 {
    let (cx, cy) = (fade.x * 1100.0, fade.y * LINES_H);
    let (rx, ry) = (0.38 * 1100.0, 0.38 * LINES_H);
    let d = (((l.x - cx) / rx).powi(2) + ((l.y - cy) / ry).powi(2)).sqrt();
    ((d - 0.58) / 0.42).clamp(0.0, 1.0)
}

/// Paints the lines over the whole window, behind the title bar too (`main` = the area under the bar). `fade` is
/// the centre of the clear zone as a fraction of the window.
pub fn paint_lines(painter: &Painter, main: Rect, t: &Theme, fade: Vec2) {
    let win = window_of(main);
    let painter = painter.with_clip_rect(win.intersect(painter.clip_rect()));
    let scale = Vec2::new(win.width() / 1100.0, win.height() / LINES_H);
    let (a1, a2) = (t.a1, t.a2);
    let opacity = t.lines;
    let origin = win.min;
    for i in 0..16 {
        let pts = contour(i);
        let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        let screen: Vec<Pos2> = pts.iter().map(|p| origin + Vec2::new(p.x * scale.x, p.y * scale.y)).collect();
        let stroke = PathStroke::new_uv(1.0_f32, move |_bounds, p: Pos2| {
            let l = Pos2::new((p.x - origin.x) / scale.x, (p.y - origin.y) / scale.y);
            // Gradient x1=0 y1=0 x2=1 y2=1 in the path's bounding box.
            let u = (l.x + 20.0) / 1140.0;
            let v = if y1 > y0 { (l.y - y0) / (y1 - y0) } else { 0.0 };
            let c = crate::theme::mix(a1, a2, ((u + v) / 2.0).clamp(0.0, 1.0));
            crate::theme::alpha(c, opacity * mask(l, fade))
        });
        painter.add(Shape::line(screen, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contours_cover_the_width() {
        for i in 0..16 {
            let c = contour(i);
            assert_eq!(c.first().unwrap().x, -20.0);
            assert_eq!(c.last().unwrap().x, 1120.0);
        }
    }

    #[test]
    fn glows_fade_to_the_base_colour() {
        let t = Theme::resolve(0.0, Color32::from_rgb(255, 0, 0), Color32::from_rgb(0, 0, 255));
        let size = Vec2::new(1100.0, 720.0);
        assert_eq!(window_color(Pos2::new(550.0, 400.0), size, &t), t.bg);
        let corner = window_color(Pos2::new(44.0, 0.0), size, &t);
        assert!(corner.r() > t.bg.r() + 40);
    }
}
