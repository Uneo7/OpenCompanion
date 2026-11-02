use eframe::egui::{self, Align, Id, Pos2, Rect, Vec2};

use super::rows::{self, Page};
use crate::app::actions::Action;
use crate::assets::icons::{self, Icon};
use crate::i18n::{t_with, tr};
use crate::theme::{self, typography::W};
use crate::views::Cx;
use crate::widgets::{button::IconButton, fmt, text, text::st};

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, p: &mut Page) {
    let tel = cx.device.telemetry();
    let status = cx.device.status();
    p.head(ui, tr("settings.status.title"), None);

    p.section(ui, tr("settings.status.measurements"));
    let r = p.block(8.0 + 40.0 + 16.0 + 40.0 + 8.0);
    let col_w = (r.width() - 16.0) / 2.0;
    let at = |i: usize| Pos2::new(r.left() + (i % 2) as f32 * (col_w + 16.0), r.top() + 8.0 + (i / 2) as f32 * 56.0);
    tile(ui, at(0), icons::CHIP, &format!("{:.0}", tel.dsp_load), Some("%"), tr("settings.status.dsp_load"));
    tile(ui, at(1), icons::BOLT, &fmt::fr(f64::from(tel.supply), 1), Some("V"), tr("settings.status.supply"));
    let therm = tr(if tel.thermal_trip { "settings.status.thermal.on" } else { "settings.status.inactive" });
    tile(ui, at(2), icons::THERMO, therm, None, tr("settings.status.thermal_protection"));
    let clips = format!("{} · {}", tel.clips.0, tel.clips.1);
    let right = tile(ui, at(3), icons::WAVE, &clips, Some("A · B"), tr("settings.status.amp_clips"));
    let reset = Rect::from_min_size(Pos2::new(right + 4.0 + 2.0, at(3).y + 1.0), Vec2::splat(22.0));
    let resp = IconButton::new(icons::ROTATE).tip(tr("settings.status.tip.reset"), None).show(ui, reset, Id::new("clip-reset"));
    if resp.clicked() {
        cx.act(Action::ResetClips);
    }

    p.section(ui, tr("settings.status.components"));
    let rows_ = p.list(ui, &[60.0; 4]);
    // The USB mode the speaker is in (set in Settings › Device); the received stream when it is unknown.
    let conn = match cx.device.mode() {
        Some(m) => crate::i18n::t_with("settings.status.conn", &[("fmt", &fmt::mode(m))]),
        None => fmt::stream(status.stream),
    };
    // Without a pod: "Absente", no knob or mute details. An invalid knob reading shows no bass value.
    let bass = if status.pod_knob_ok { fmt::frs(f64::from(status.pod_bass), 0) } else { "·".to_owned() };
    let pod = t_with(
        "settings.status.pod.line",
        &[
            ("vol", &cx.device.volume()),
            ("bass", &bass),
            ("mute", &tr(if cx.device.muted() { "settings.status.pod.mute.on" } else { "settings.status.pod.mute.off" })),
            ("headphones", &tr(if status.headphones { "settings.status.pod.hp.on" } else { "settings.status.pod.hp.off" })),
        ],
    );
    let fw = t_with("settings.status.fw.line", &[("usb", &status.firmware_usb), ("dsp", &status.firmware_dsp)]);
    let faults = if tel.faults == 0 { tr("settings.status.none").to_owned() } else { tel.faults.to_string() };
    let items: [(&str, &str, &str, bool); 4] = [
        (tr("settings.status.connection"), &conn, tr("settings.status.connected"), true),
        (tr("settings.status.faults"), tr("settings.status.faults_desc"), &faults, false),
        if status.pod_present {
            (tr("settings.status.pod"), &pod, tr("settings.status.present"), false)
        } else {
            (tr("settings.status.pod"), tr("settings.status.pod.none"), tr("settings.status.absent"), false)
        },
        (tr("settings.status.firmware"), &fw, tr("settings.status.up_to_date"), false),
    ];
    for (row, (title, sub, v, dot)) in rows_.iter().zip(items) {
        rows::row_text(ui, row.left(), row.center().y, title, Some(sub), None);
        rows::value_text(ui, row.right(), row.center().y, v, dot);
    }
}

fn tile(ui: &egui::Ui, min: Pos2, icon: Icon, value: &str, unit: Option<&str>, caption: &str) -> f32 {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let ti = Rect::from_min_size(min + Vec2::new(0.0, 2.0), Vec2::splat(36.0));
    ui.painter().rect_filled(ti, 11.0, t.field);
    icon.paint(ui.painter(), ti.center(), 19.0, t.muted);
    let x = ti.right() + 12.0;
    let vstyle = st::inter_(20.0, W::SemiBold).tracking(-0.02);
    let vr = text::line(ui.painter(), value, &vstyle, t.text, x, min.y, 24.0, Align::Min);
    let mut right = vr.right();
    if let Some(u) = unit {
        let us = st::mono_(11.0, W::Regular);
        let base = min.y + text::baseline_in_box(&ctx, &vstyle, 24.0);
        let lt = base - text::baseline_in_box(&ctx, &us, 16.0);
        right = text::line(ui.painter(), u, &us, t.muted, vr.right() + 4.0, lt, 16.0, Align::Min).right();
    }
    text::line(ui.painter(), caption, &st::inter_(12.0, W::Regular), t.muted, x, min.y + 24.0, 16.0, Align::Min);
    right
}
