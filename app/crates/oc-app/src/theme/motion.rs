use eframe::egui::{Color32, Context, Id};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ease(pub f32, pub f32, pub f32, pub f32);

pub const EASE: Ease = Ease(0.25, 0.1, 0.25, 1.0);
pub const EASE_OUT: Ease = Ease(0.0, 0.0, 0.58, 1.0);
pub const SPRING: Ease = Ease(0.2, 0.8, 0.2, 1.0);
/// `1 − (1 − t)³`, used by the background fade.
pub const CUBIC_OUT: Ease = Ease(0.33, 1.0, 0.68, 1.0);

impl Ease {
    pub fn at(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        if x == 0.0 || x == 1.0 {
            return x;
        }
        let Ease(x1, y1, x2, y2) = self;
        let bez = |t: f32, p1: f32, p2: f32| {
            let u = 1.0 - t;
            3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
        };
        let dbez = |t: f32, p1: f32, p2: f32| {
            let u = 1.0 - t;
            3.0 * u * u * p1 + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
        };
        // Newton on x(t) = x, then bisection if it does not converge.
        let mut t = x;
        for _ in 0..8 {
            let err = bez(t, x1, x2) - x;
            let d = dbez(t, x1, x2);
            if err.abs() < 1e-5 {
                return bez(t, y1, y2);
            }
            if d.abs() < 1e-6 {
                break;
            }
            t = (t - err / d).clamp(0.0, 1.0);
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            t = (lo + hi) / 2.0;
            if bez(t, x1, x2) < x {
                lo = t;
            } else {
                hi = t;
            }
        }
        bez(t, y1, y2)
    }
}

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    start: f64,
    secs: f32,
}

impl Tween {
    fn value(&self, now: f64, ease: Ease) -> f32 {
        if self.secs <= 0.0 {
            return self.to;
        }
        let k = ((now - self.start) as f32 / self.secs).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * ease.at(k)
    }
}

#[derive(Clone, Copy)]
struct Reduced(bool);

/// Turns animations off (instant state changes), like `prefers-reduced-motion: reduce`.
pub fn set_reduced(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(Id::NULL.with("reduced-motion"), Reduced(on)));
}

pub fn reduced(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Reduced>(Id::NULL.with("reduced-motion"))).is_some_and(|r| r.0)
}

/// Animates towards `target` over `secs`, restarting from the current value when the target changes.
pub fn tween(ctx: &Context, id: Id, target: f32, secs: f32, ease: Ease) -> f32 {
    tween_from(ctx, id, target, target, secs, ease)
}

pub fn tween_from(ctx: &Context, id: Id, initial: f32, target: f32, secs: f32, ease: Ease) -> f32 {
    let now = ctx.input(|i| i.time);
    let secs = if reduced(ctx) { 0.0 } else { secs };
    let prev = ctx.data(|d| d.get_temp::<Tween>(id));
    let tw = match prev {
        Some(tw) if tw.to == target => tw,
        Some(tw) => Tween { from: tw.value(now, ease), to: target, start: now, secs },
        None => Tween { from: initial, to: target, start: now, secs },
    };
    if prev.is_none_or(|p| p.to != target) {
        ctx.data_mut(|d| d.insert_temp(id, tw));
    }
    if now < tw.start + f64::from(tw.secs) {
        ctx.request_repaint();
    }
    tw.value(now, ease)
}

pub fn toggle(ctx: &Context, id: Id, on: bool, secs: f32, ease: Ease) -> f32 {
    tween(ctx, id, if on { 1.0 } else { 0.0 }, secs, ease)
}

pub fn hover(ctx: &Context, id: Id, on: bool) -> f32 {
    toggle(ctx, id.with("hover"), on, 0.15, EASE)
}

pub fn color(ctx: &Context, id: Id, target: Color32, secs: f32, ease: Ease) -> Color32 {
    let c = target.to_array();
    let ch = |i: usize| tween(ctx, id.with(i), f32::from(c[i]), secs, ease).round() as u8;
    Color32::from_rgba_premultiplied(ch(0), ch(1), ch(2), ch(3))
}

/// Progress (0 → 1 over `secs`) since `key` last changed: one-shot entrance animations.
pub fn since(ctx: &Context, id: Id, key: u64, secs: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    if reduced(ctx) {
        return 1.0;
    }
    let (k, start) = ctx.data(|d| d.get_temp::<(u64, f64)>(id)).unwrap_or((key.wrapping_add(1), now));
    let start = if k == key {
        start
    } else {
        ctx.data_mut(|d| d.insert_temp(id, (key, now)));
        now
    };
    let p = ((now - start) as f32 / secs).clamp(0.0, 1.0);
    if p < 1.0 {
        ctx.request_repaint();
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_endpoints_and_shape() {
        for e in [EASE, EASE_OUT, SPRING, CUBIC_OUT] {
            assert_eq!(e.at(0.0), 0.0);
            assert_eq!(e.at(1.0), 1.0);
            let mut last = 0.0;
            for i in 1..20 {
                let v = e.at(i as f32 / 20.0);
                assert!(v >= last - 1e-4, "monotonic");
                last = v;
            }
        }
        // Linear curve stays linear; cubic-out matches 1 − (1 − t)³ closely.
        assert!((Ease(0.0, 0.0, 1.0, 1.0).at(0.3) - 0.3).abs() < 1e-3);
        assert!((CUBIC_OUT.at(0.5) - 0.875).abs() < 0.02);
    }
}
