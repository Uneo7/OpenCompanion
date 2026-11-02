use eframe::egui::{
    epaint::{Mesh, Vertex, WHITE_UV},
    Color32, CornerRadius, Painter, Pos2, Rect, Shadow, Shape, Stroke, Vec2,
};

/// Fills a convex polygon with a per-point colour (premultiplied), anti-aliased with a one-pixel feather.
pub fn fill_convex(painter: &Painter, points: &[Pos2], color_at: impl Fn(Pos2) -> Color32) {
    let n = points.len();
    if n < 3 {
        return;
    }
    let feather = 1.0 / painter.ctx().pixels_per_point();
    let mut mesh = Mesh::default();
    let centroid = points.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / n as f32;
    // Inner ring (half a pixel inside) at full colour, outer ring (half a pixel outside) transparent.
    for i in 0..n {
        let prev = points[(i + n - 1) % n];
        let next = points[(i + 1) % n];
        let p = points[i];
        let d1 = (p - prev).normalized();
        let d2 = (next - p).normalized();
        let mut normal = (Vec2::new(d1.y, -d1.x) + Vec2::new(d2.y, -d2.x)).normalized();
        if normal.dot(p - centroid.to_pos2()) < 0.0 {
            normal = -normal;
        }
        let c = color_at(p);
        mesh.vertices.push(Vertex { pos: p - normal * feather * 0.5, uv: WHITE_UV, color: c });
        mesh.vertices.push(Vertex { pos: p + normal * feather * 0.5, uv: WHITE_UV, color: Color32::TRANSPARENT });
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (ii, io, ji, jo) = (2 * i as u32, 2 * i as u32 + 1, 2 * j as u32, 2 * j as u32 + 1);
        mesh.add_triangle(ii, io, jo);
        mesh.add_triangle(ii, jo, ji);
    }
    for i in 1..n - 1 {
        mesh.add_triangle(0, 2 * i as u32, 2 * (i + 1) as u32);
    }
    painter.add(Shape::mesh(mesh));
}

pub fn shadow(painter: &Painter, rect: Rect, radius: f32, dy: f32, blur: f32, spread: f32, color: Color32) {
    let s = Shadow {
        offset: [0, dy.round() as i8],
        blur: blur.round().clamp(0.0, 255.0) as u8,
        spread: spread.round().clamp(0.0, 255.0) as u8,
        color,
    };
    painter.add(s.as_shape(rect, CornerRadius::from(radius)));
}

pub fn dashed_circle(painter: &Painter, center: Pos2, r: f32, stroke: Stroke, dash: f32, gap: f32) {
    let n = 48;
    let pts: Vec<Pos2> =
        (0..=n).map(|i| center + Vec2::angled(i as f32 / n as f32 * std::f32::consts::TAU) * r).collect();
    painter.extend(Shape::dashed_line(&pts, stroke, dash, gap));
}

pub fn faded(painter: &Painter, opacity: f32) -> Painter {
    let mut p = painter.clone();
    p.multiply_opacity(opacity);
    p
}

/// Circle split along the diagonal (`linear-gradient(135deg, c1 50%, c2 50%)`): `c1` top-left, `c2` bottom-right.
pub fn split_circle(painter: &Painter, center: Pos2, r: f32, c1: Color32, c2: Color32) {
    let half = |start: f32, c: Color32| {
        let n = 24;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| center + Vec2::angled((start + 180.0 * i as f32 / n as f32).to_radians()) * r)
            .collect();
        fill_convex(painter, &pts, |_| c);
    };
    // Screen angles: 135° points bottom-left, so the half from 135° to 315° (through 225°, top-left) is c1.
    half(135.0, c1);
    half(-45.0, c2);
}
