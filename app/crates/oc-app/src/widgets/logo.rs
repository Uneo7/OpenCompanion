use eframe::egui::{epaint::CubicBezierShape, pos2, Color32, Painter, Rect, Stroke, StrokeKind};

const BG: Color32 = Color32::from_rgb(0x0f, 0x1b, 0x2d);
const CURVE: Color32 = Color32::from_rgb(0x5b, 0x9d, 0xff);

pub fn paint(painter: &Painter, rect: Rect) {
    let s = rect.width() / 64.0;
    let at = |x: f32, y: f32| pos2(rect.left() + x * s, rect.top() + y * s);
    let square = Rect::from_min_max(at(1.0, 1.0), at(63.0, 63.0));
    painter.rect_filled(square, 15.0 * s, BG);
    painter.rect_stroke(square.shrink(0.5 * s), 14.5 * s, Stroke::new(s, Color32::from_white_alpha(20)), StrokeKind::Middle);
    // <g transform="translate(9 9) scale(.72)"><path d="M6 46c10 0 14-24 26-24s14 16 26 16"/>
    let p = |x: f32, y: f32| at(9.0 + x * 0.72, 9.0 + y * 0.72);
    let stroke = Stroke::new(6.5 * 0.72 * s, CURVE);
    for pts in [
        [p(6.0, 46.0), p(16.0, 46.0), p(20.0, 22.0), p(32.0, 22.0)],
        [p(32.0, 22.0), p(44.0, 22.0), p(46.0, 38.0), p(58.0, 38.0)],
    ] {
        painter.add(CubicBezierShape::from_points_stroke(pts, false, Color32::TRANSPARENT, stroke));
    }
    painter.circle_filled(p(6.0, 46.0), 6.5 * 0.36 * s, CURVE);
    painter.circle_filled(p(58.0, 38.0), 6.5 * 0.36 * s, CURVE);
    painter.circle(p(32.0, 22.0), 7.5 * 0.72 * s, Color32::WHITE, Stroke::new(3.5 * 0.72 * s, BG));
}
