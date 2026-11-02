//! Controls register a request while hovered; the app paints the last one on top of everything.

use eframe::egui::{Align, Context, Id, LayerId, Order, Rect, Response, Vec2};

use super::text::{self, st};
use crate::theme::{self, typography::W};

#[derive(Clone)]
struct Request {
    anchor: Rect,
    text: String,
    kbd: Option<String>,
    /// Requested from the topmost layer (an open dialog): shown even while a dialog is open.
    top: bool,
}

fn key() -> Id {
    Id::NULL.with("oc-tooltip")
}

pub fn attach(response: &Response, tip: &str, kbd: Option<&str>) {
    if response.hovered() && !response.ctx.input(|i| i.pointer.any_down()) {
        request(&response.ctx, response.rect, tip, kbd);
    }
}

pub fn request(ctx: &Context, anchor: Rect, tip: &str, kbd: Option<&str>) {
    let r = Request { anchor, text: tip.to_owned(), kbd: kbd.map(str::to_owned), top: false };
    ctx.data_mut(|d| d.insert_temp(key(), Some(r)));
}

/// [`attach`] for a control inside the open dialog: not hidden by the dialog itself.
pub fn attach_top(response: &Response, tip: &str) {
    if response.hovered() && !response.ctx.input(|i| i.pointer.any_down()) {
        let r = Request { anchor: response.rect, text: tip.to_owned(), kbd: None, top: true };
        response.ctx.data_mut(|d| d.insert_temp(key(), Some(r)));
    }
}

/// Paints this frame's tooltip (unless `suppressed`, e.g. while a menu is open) and clears the request.
pub fn paint(ctx: &Context, window: Rect, suppressed: bool) {
    let Some(r) = ctx.data_mut(|d| d.get_temp::<Option<Request>>(key()).flatten()) else { return };
    ctx.data_mut(|d| d.insert_temp::<Option<Request>>(key(), None));
    if suppressed && !r.top {
        return;
    }
    let t = theme::get(ctx);
    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, key()));
    let style = st::inter_(12.0, W::Medium);
    let kstyle = st::mono_(11.0, W::Medium);
    let tw = text::width(ctx, &r.text, &style);
    let kw = r.kbd.as_deref().map_or(0.0, |k| 6.0 + text::width(ctx, k, &kstyle));
    let w = 8.0 + tw + kw + 8.0;
    let x = (r.anchor.center().x - w / 2.0).clamp(window.left() + 8.0, window.right() - 8.0 - w);
    let rect = Rect::from_min_size(eframe::egui::pos2(x, r.anchor.bottom() + 8.0), Vec2::new(w, 26.0));
    painter.rect_filled(rect, 6.0, t.text);
    text::line(&painter, &r.text, &style, t.bg, rect.left() + 8.0, rect.top(), 26.0, Align::Min);
    if let Some(k) = &r.kbd {
        text::line(&painter, k, &kstyle, theme::alpha(t.bg, 0.7), rect.left() + 8.0 + tw + 6.0, rect.top(), 26.0, Align::Min);
    }
}
