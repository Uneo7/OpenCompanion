use eframe::egui::{Align, Context, Id, LayerId, Order, Pos2, Rect, Vec2};

use super::text::{self, st};
use crate::assets::icons;
use crate::theme::{self, motion, typography::W};

const SHOWN_S: f64 = 2.0;

#[derive(Default)]
pub struct Toast {
    message: String,
    shown_at: Option<f64>,
    serial: u64,
}

impl Toast {
    pub fn show(&mut self, ctx: &Context, message: impl Into<String>) {
        self.message = message.into();
        self.shown_at = Some(ctx.input(|i| i.time));
        self.serial += 1;
    }

    pub fn paint(&mut self, ctx: &Context, window: Rect) {
        let Some(at) = self.shown_at else { return };
        let now = ctx.input(|i| i.time);
        let on = now - at < SHOWN_S;
        let id = Id::new("oc-toast");
        let k = motion::tween_from(ctx, id.with(self.serial), 0.0, if on { 1.0 } else { 0.0 }, 0.16, motion::EASE);
        if on {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(SHOWN_S - (now - at)));
        } else if k <= 0.0 {
            self.shown_at = None;
            return;
        }
        let t = theme::get(ctx);
        let mut painter = ctx.layer_painter(LayerId::new(Order::Foreground, id));
        painter.set_opacity(k);
        let style = st::sec(W::Medium);
        // Never wider than the window: a long message is shortened.
        let msg = text::elide(ctx, &self.message, &style, window.width() - 48.0 - 12.0 - 16.0 - 8.0 - 16.0);
        let tw = text::width(ctx, &msg, &style);
        let w = 12.0 + 16.0 + 8.0 + tw + 16.0;
        let top = window.bottom() - 24.0 - 36.0 + 8.0 * (1.0 - k);
        let rect = Rect::from_min_size(Pos2::new(window.center().x - w / 2.0, top), Vec2::new(w, 36.0));
        painter.rect_filled(rect, 18.0, t.text);
        icons::CHECK_CIRCLE.paint(&painter, Pos2::new(rect.left() + 12.0 + 8.0, rect.center().y), 16.0, t.toast_ok);
        text::line(&painter, &msg, &style, t.bg, rect.left() + 36.0, rect.top(), 36.0, Align::Min);
    }
}
