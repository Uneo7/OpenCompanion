//! Power cycle page, after a completed restore: the stock DSP still runs from its boot under our USB controller
//! (no stock handshake: the pod knob does nothing, the volume is at its maximum) until the speaker is unplugged
//! from the mains. The page follows the speaker: waiting for it to leave, then to come back; "C'est fait" is
//! the manual way out.

use eframe::egui::{self, Align, Id, Pos2};

use crate::assets::icons;
use crate::i18n::tr;
use crate::theme::{self, typography::W};
use crate::widgets::{button, spinner::spinner, text, text::st};

const ICON: f32 = 112.0;
const TITLE_H: f32 = 52.0;
const LINE_H: f32 = 26.0;

/// Draws the page in `area`; `unplugged`: the speaker has left (waiting for it to come back). True when
/// "C'est fait" is clicked.
pub fn show(ui: &mut egui::Ui, area: egui::Rect, unplugged: bool) -> bool {
    let t = theme::get(ui.ctx());
    let block = ICON + 36.0 + TITLE_H + 10.0 + LINE_H + 32.0 + LINE_H + 32.0 + 40.0;
    let x = area.left();
    let mut y = area.center().y - block / 2.0 - 12.0;

    let icon = if unplugged { icons::PLUG } else { icons::UNPLUG };
    icon.paint(ui.painter(), Pos2::new(x + ICON / 2.0, y + ICON / 2.0), ICON, t.a1);
    y += ICON + 36.0;

    let title = st::inter_(42.0, W::SemiBold).tracking(-0.02);
    text::line(ui.painter(), tr("power.title"), &title, t.text, x, y, TITLE_H, Align::Min);
    y += TITLE_H + 10.0;
    text::line(ui.painter(), tr("power.sub"), &st::inter_(19.0, W::Regular), t.text, x, y, LINE_H, Align::Min);
    y += LINE_H + 32.0;

    spinner(ui, Pos2::new(x + 8.0, y + LINE_H / 2.0), 8.0);
    let status = tr(if unplugged { "power.wait_replug" } else { "power.wait_unplug" });
    text::line(ui.painter(), status, &st::inter_(15.0, W::Medium), t.muted, x + 28.0, y, LINE_H, Align::Min);
    y += LINE_H + 32.0;

    let done = button::Button::new(tr("power.done")).icon(icons::CHECK);
    done.show(ui, Pos2::new(x, y), Id::new("power-done")).clicked()
}
