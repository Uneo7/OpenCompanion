use eframe::egui::{self, Id, Key, Pos2, Rect, ScrollArea, Vec2};

use super::rows::{self, Page};
use crate::app::actions::Action;
use crate::assets::icons;
use crate::i18n::{lookup, tr, Lang};
use crate::theme::{self, typography::W};
use crate::model::{Connection, FirmwarePart};
use crate::views::Cx;
use crate::widgets::{button, input, text, text::st};

const CONSOLE_H: f32 = 162.0;

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, p: &mut Page) {
    let t = theme::get(ui.ctx());
    p.head(ui, tr("settings.advanced.title"), Some((tr("settings.advanced.desc"), None)));

    p.section(ui, tr("settings.advanced.console_dsp"));
    let r = p.block(CONSOLE_H + 8.0 + 36.0);
    let console = Rect::from_min_size(r.min, Vec2::new(r.width(), CONSOLE_H));
    ui.painter().rect_filled(console, 8.0, t.field);
    let inner = console.shrink(12.0);
    let mono = st::mono_(12.0, W::Regular);
    // The untouched console shows its intro in the current language.
    let fr_intro = lookup(Lang::Fr, "console.intro").map(|s| format!("{s}\n"));
    let log = if fr_intro.as_deref() == Some(cx.ui.settings.console.as_str()) {
        tr("console.intro").to_owned()
    } else {
        cx.ui.settings.console.clone()
    };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    ScrollArea::vertical()
        .id_salt("console")
        .max_height(inner.height())
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            let g = text::paragraph_galley(ui.ctx(), log.trim_end_matches('\n'), &mono, t.muted, inner.width(), 18.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(inner.width(), g.rows.len() as f32 * 18.0), egui::Sense::hover());
            let natural = ui.ctx().fonts(|f| f.row_height(&mono.font));
            ui.painter().galley(rect.min + Vec2::new(0.0, (18.0 - natural) / 2.0), g, t.muted);
        });

    let y = console.bottom() + 8.0;
    let send = button::Button::new(tr("settings.advanced.send")).icon(icons::ENTER);
    let sw = send.size(ui.ctx()).x;
    let field = Rect::from_min_size(Pos2::new(r.left(), y), Vec2::new(r.width() - sw - 8.0, 36.0));
    let resp = input::field(
        ui,
        field,
        Id::new("console-in"),
        &mut cx.ui.settings.console_input,
        tr("settings.advanced.aria.console_input"),
        st::mono_(13.0, W::Regular).font,
        None,
    );
    let enter = resp.lost_focus() && ui.ctx().input(|i| i.key_pressed(Key::Enter));
    if send.show(ui, Pos2::new(field.right() + 8.0, y), Id::new("console-send")).clicked() || enter {
        cx.act(Action::ConsoleSend);
        if enter {
            resp.request_focus();
        }
    }

    restarts(ui, cx, p);
}

/// "Restart": one row per chip, a Restart button (asks first), a spinner while it restarts. Disabled
/// during an install, a mode switch or another restart, and without a ready speaker.
fn restarts(ui: &mut egui::Ui, cx: &mut Cx, p: &mut Page) {
    p.section(ui, tr("settings.advanced.restart.section"));
    let rows_ = p.list(ui, &[60.0, 60.0]);
    let running = cx.device.restarting();
    let free = running.is_none() && cx.device.mode_switching().is_none() && matches!(cx.device.connection(), Connection::Ready | Connection::Simulated);
    let parts = [
        (FirmwarePart::Dsp, "settings.advanced.restart.dsp", icons::CHIP),
        (FirmwarePart::Usb, "settings.advanced.restart.usb", icons::USB),
    ];
    for (r, (part, title, icon)) in rows_.iter().zip(parts) {
        let x = rows::row_icon(ui, *r, icon);
        let hint = (running == Some(part)).then(|| tr("settings.advanced.restart.running"));
        rows::row_text(ui, x, r.center().y, tr(title), hint, None);
        if running == Some(part) {
            crate::widgets::spinner::spinner(ui, Pos2::new(r.right() - 8.0, r.center().y), 8.0);
            continue;
        }
        let b = button::Button::new(tr("settings.advanced.restart.ok")).small().enabled(free);
        let at = Pos2::new(r.right() - b.size(ui.ctx()).x, r.center().y - b.size(ui.ctx()).y / 2.0);
        if b.show(ui, at, Id::new(("restart", title))).clicked() && free {
            cx.act(Action::AskRestart(part));
        }
    }
}
