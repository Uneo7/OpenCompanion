use eframe::egui::{self, Align, Id, Pos2, Rect, Vec2};
use oc_device::install::{Chip, Flow, UpdateCheck};

use super::rows::{self, Page};
use crate::app::actions::Action;
use crate::assets::icons;
use crate::i18n::{t_with, tr};
use crate::model::FirmwarePart;
use crate::theme::{self, typography::W};
use crate::model::format::version_label;
use crate::views::{install, Cx};
use crate::widgets::{button, text, text::st};

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, p: &mut Page) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let status = cx.device.status();
    // What the update check found (the hardware only; the simulated speaker is always up to date).
    let check = cx.device.updates();
    let head = match &check {
        UpdateCheck::Running => tr("settings.firmware.checking").to_owned(),
        UpdateCheck::Failed(e) => t_with("settings.firmware.check_failed", &[("error", e)]),
        c if c.available() => tr("settings.firmware.available").to_owned(),
        _ => tr("settings.firmware.up_to_date").to_owned(),
    };
    p.head_status(ui, tr("settings.firmware.firmware"), &head, matches!(check, UpdateCheck::Running));

    p.section(ui, tr("settings.firmware.updates"));
    // "Version 0.3.0", or "Version 0.3.0 → 0.3.1" when the app has another build for that chip.
    let line = |chip: Chip, running: &str| match check.chip(chip) {
        Some(u) if u.available => {
            let (from, to) = (crate::model::format::from_label(&u.from, u.from_stock, u.from_probable), version_label(&u.to, false));
            t_with("settings.firmware.version_to", &[("from", &from), ("to", &to)])
        }
        _ => t_with("settings.firmware.version", &[("v", &running)]),
    };
    let parts = [
        (FirmwarePart::Usb, Chip::Usb, "settings.firmware.usb", line(Chip::Usb, &status.firmware_usb)),
        (FirmwarePart::Dsp, Chip::Dsp, "settings.firmware.dsp", line(Chip::Dsp, &status.firmware_dsp)),
    ];
    let progress: Vec<_> = parts.iter().map(|(part, ..)| cx.device.update_progress(*part)).collect();
    // Tall rows: 16 + 20 + 18 (+ 8 + 4 for a running update's bar) + 16.
    let mut heights = vec![60.0];
    heights.extend(progress.iter().map(|pr| 16.0 + 38.0 + if pr.is_some() { 12.0 } else { 0.0 } + 16.0));
    let rows_ = p.list(ui, &heights);

    // Installer entry: "Update" when the check found something, else the assistant (the check runs by
    // itself: at each connection, on opening this page or the installer, after each install).
    let r = rows_[0];
    let x = rows::row_icon(ui, r, icons::SPARKLE);
    rows::row_text(ui, x, r.center().y, tr("inst.entry"), None, None);
    let label = tr(if check.available() { "inst.start.update" } else { "inst.entry.btn" });
    let open = button::Button::new(label).small().kind(button::Kind::Primary);
    let w = open.size(&ctx).x;
    if open.show(ui, Pos2::new(r.right() - w, r.center().y - 15.0), Id::new("inst-open")).clicked() {
        install::open(&ctx, cx, Flow::Update);
    }

    for (k, ((_, chip, title, sub), r)) in parts.iter().zip(rows_[1..].iter()).enumerate() {
        let top = r.top() + 16.0;
        text::line(ui.painter(), tr(title), &st::body(W::Medium), t.text, r.left(), top, 20.0, Align::Min);
        let sub = match progress[k] {
            Some(pr) => format!("{}… {} %", pr.step, pr.percent),
            None => sub.clone(),
        };
        text::line(ui.painter(), &sub, &st::sec(W::Regular), t.muted, r.left(), top + 20.0, 18.0, Align::Min);
        let btn = button::Button::new(tr("settings.firmware.install")).icon(icons::UPLOAD).small();
        let size = btn.size(&ctx);
        let bx = r.right() - size.x;
        if let Some(pr) = progress[k] {
            let bar = Rect::from_min_size(Pos2::new(r.left(), top + 38.0 + 8.0), Vec2::new(bx - 16.0 - r.left(), 4.0));
            ui.painter().rect_filled(bar, 2.0, t.track);
            let f = Rect::from_min_size(bar.min, Vec2::new(bar.width() * f32::from(pr.percent) / 100.0, 4.0));
            ui.painter().rect_filled(f, 2.0, t.fill);
        }
        if btn.show(ui, Pos2::new(bx, r.center().y - 15.0), Id::new(("fw", k))).clicked() {
            cx.ui.pick = Some(crate::app::files::Pick::Firmware(*chip));
        }
    }

    if cfg!(feature = "release-updates") {
        if let Some(rs) = cx.device.installer().and_then(|i| i.release_status()) {
            let r = p.block(18.0);
            text::line(ui.painter(), &install::explain::release_line(&rs), &st::sec(W::Regular), t.muted, r.left(), r.top(), 18.0, Align::Min);
        }
    }

    p.section(ui, tr("settings.firmware.backup"));
    let r = p.list(ui, &[60.0])[0];
    let x = rows::row_icon(ui, r, icons::DRIVE);
    let backup = install::backup::backup_state(cx);
    let release = cx.device.installer().and_then(|i| i.release_status());
    let stock = cx.device.connection() == crate::model::Connection::Stock;
    let restore_why = install::explain::restore_blocker(&backup, stock, cx.device.restore_pending(), release.as_ref());
    // Saved but useless (the speaker already runs the Bose firmware): the row says why the restore is off.
    let sub = match (&backup, &restore_why) {
        (oc_device::install::BackupState::Saved { .. }, Some(why)) => format!("{} · {why}", install::backup::status_text(&backup)),
        (oc_device::install::BackupState::Failed(e), _) => install::explain::backup_failure(e, release.as_ref()),
        _ => install::backup::status_text(&backup),
    };
    rows::row_text(ui, x, r.center().y, tr("settings.firmware.backup_desc"), Some(&sub), None);
    let save = button::Button::new(tr("settings.firmware.backup_now")).small();
    let sw = save.size(&ctx).x;
    if save.show(ui, Pos2::new(r.right() - sw, r.center().y - 15.0), Id::new("bk-now")).clicked() && !install::backup::request(cx) {
        cx.act(Action::Backup);
    }
    let blocked = restore_why.is_some() || install::backup::blocks(&Flow::Restore, &backup);
    let restore = button::Button::new(tr("settings.firmware.restore")).small().kind(button::Kind::Danger).enabled(!blocked);
    let rw = restore.size(&ctx).x;
    if restore.show(ui, Pos2::new(r.right() - sw - 16.0 - rw, r.center().y - 15.0), Id::new("bk-restore")).clicked() {
        install::open(&ctx, cx, Flow::Restore);
    }
}

