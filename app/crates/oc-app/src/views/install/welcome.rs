//! Welcome page: shown whenever the speaker runs the original Bose firmware. It invites to install: the logo,
//! the title, what OpenCompanion brings and one button that opens the installer.

use eframe::egui::{self, Align, Id, Pos2, Rect, Vec2};

use crate::assets::icons;
use crate::i18n::tr;
use crate::theme::{self, typography::W};
use crate::widgets::{button, logo, text, text::st};

const LOGO: f32 = 72.0;
const TITLE_H: f32 = 52.0;
const LINE_H: f32 = 26.0;
const BUTTON_H: f32 = 40.0;

/// Draws the page in `area`; true when "Installer OpenCompanion" is clicked.
pub fn show(ui: &mut egui::Ui, area: Rect) -> bool {
    let t = theme::get(ui.ctx());
    let block = LOGO + 32.0 + TITLE_H + 14.0 + LINE_H + 36.0 + BUTTON_H;
    let x = area.left();
    let mut y = area.center().y - block / 2.0 - 12.0;

    logo::paint(ui.painter(), Rect::from_min_size(Pos2::new(x, y), Vec2::splat(LOGO)));
    y += LOGO + 32.0;

    let title = st::inter_(42.0, W::SemiBold).tracking(-0.02);
    text::line(ui.painter(), tr("onboard.title"), &title, t.text, x, y, TITLE_H, Align::Min);
    y += TITLE_H + 14.0;

    text::line(ui.painter(), tr("onboard.welcome.pitch"), &st::inter_(17.0, W::Regular), t.text, x, y, LINE_H, Align::Min);
    y += LINE_H + 36.0;

    let go = button::Button::new(tr("onboard.welcome.install")).icon(icons::IMPORT).kind(button::Kind::Primary);
    go.show(ui, Pos2::new(x, y), Id::new("onboard-install")).clicked()
}
