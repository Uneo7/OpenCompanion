use eframe::egui::{Context, Rect};

use super::App;
use crate::theme::motion;
use crate::widgets::{dialog, menu, tooltip};

impl App {
    pub(super) fn overlays(&mut self, ctx: &Context, window: Rect) {
        if let Some(m) = self.ui.menu.clone() {
            match menu::show(ctx, &m, window) {
                menu::Outcome::Stay => {}
                menu::Outcome::Close => self.ui.menu = None,
                menu::Outcome::Pick(a) => {
                    self.ui.menu = None;
                    self.ui.act(a);
                }
            }
        }

        if let Some(c) = &mut self.ui.closing {
            let now = ctx.input(|i| i.time);
            let k = if motion::reduced(ctx) { 0.0 } else { 1.0 - motion::EASE.at(((now - c.since) / 0.16) as f32) };
            if k <= 0.0 {
                self.ui.closing = None;
            } else {
                ctx.request_repaint();
                dialog::show(ctx, &mut c.dialog, window, k, true);
            }
        }

        if let Some(mut d) = self.ui.dialog.take() {
            let k = dialog::progress(ctx, d.serial, true);
            let outcome = dialog::show(ctx, &mut d, window, k, false);
            self.ui.dialog = Some(d);
            match outcome {
                dialog::Outcome::Stay => {}
                dialog::Outcome::Cancel => self.close_dialog(ctx),
                dialog::Outcome::Ok => {
                    if let Some(d) = self.ui.dialog.clone() {
                        if self.apply(ctx, d.action.clone(), Some(&d.body)) {
                            self.close_dialog(ctx);
                        }
                    }
                }
            }
        }

        self.ui.toast.paint(ctx, window);
        // Under a menu or a dialog, only the dialog's own tooltips show.
        let busy = self.ui.menu.is_some() || self.ui.dialog.is_some();
        tooltip::paint(ctx, window, busy);
    }
}
