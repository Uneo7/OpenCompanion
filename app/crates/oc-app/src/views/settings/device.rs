use eframe::egui::{self, Id, Pos2};

use super::rows::{self, Page};
use crate::app::actions::Action;
use crate::i18n::tr;
use crate::model::{DeviceMode, LinkState, StandbyDelay};
use oc_device::console::options::THRESHOLD_STEPS;
use crate::views::Cx;
use crate::widgets::{
    fmt,
    menu::{Anchor, Entry, Meta},
    switch,
};

fn standby_label(s: StandbyDelay) -> &'static str {
    tr(match s {
        StandbyDelay::Off => "settings.device.standby.never",
        StandbyDelay::Stock => "settings.device.standby.2min",
        StandbyDelay::Min5 => "settings.device.standby.5min",
        StandbyDelay::Min10 => "settings.device.standby.10min",
        StandbyDelay::Min20 => "settings.device.standby.20min",
        StandbyDelay::Min30 => "settings.device.standby.30min",
        StandbyDelay::Min60 => "settings.device.standby.60min",
    })
}

const STANDBY_MENU: [StandbyDelay; 7] = [
    StandbyDelay::Off,
    StandbyDelay::Stock,
    StandbyDelay::Min5,
    StandbyDelay::Min10,
    StandbyDelay::Min20,
    StandbyDelay::Min30,
    StandbyDelay::Min60,
];

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, p: &mut Page) {
    p.head(ui, tr("settings.device.title"), None);
    let s = cx.device.settings().clone();

    // Audio USB: one "Mode" select. A switch re-enumerates the speaker (several seconds): the row shows a spinner
    // and "Changement de mode…", the select is disabled, until the speaker is back in the new mode.
    p.section(ui, tr("settings.device.usb_audio"));
    let r = p.list(ui, &[60.0])[0];
    let target = cx.device.mode_switching();
    let switching = target.is_some() || cx.device.status().link == LinkState::Reconnecting;
    let hint = switching.then(|| tr("settings.device.mode.switching"));
    rows::row_text(ui, r.left(), r.center().y, tr("settings.device.mode"), hint, None);
    let mode = cx.device.mode();
    let label = mode.map_or("·", fmt::mode);
    if cx.device.can_set_mode() && !switching {
        let key = Id::new("usb-mode");
        let open = cx.menu_open(key);
        let resp = rows::select(ui, r.right(), r.center().y, key.with("btn"), label, open, None);
        if resp.clicked() {
            let entries = DeviceMode::ALL
                .iter()
                .map(|m| {
                    let e = Entry::plain(fmt::mode(*m), Action::SetMode(*m));
                    if Some(*m) == mode { e.meta(Meta::Check) } else { e }
                })
                .collect();
            cx.toggle_menu(key, Anchor::Rect(resp.rect), false, entries);
        }
    } else if switching {
        let shown = target.map_or(label, fmt::mode);
        let tr_ = rows::value_text_rect(ui, r.right(), r.center().y, shown);
        crate::widgets::spinner::spinner(ui, egui::Pos2::new(tr_.left() - 12.0 - 8.0, r.center().y), 8.0);
    } else {
        rows::value_text(ui, r.right(), r.center().y, label, false);
    }

    // Stored in the speaker from DSP profile protocol 4 on (`OPC`); hidden with an older DSP firmware.
    if !cx.device.has_device_options() {
        return;
    }
    let o = s.options;
    p.section(ui, tr("settings.device.standby"));
    let rows_ = p.list(ui, &[60.0, 60.0]);
    let r = rows_[0];
    rows::row_text(ui, r.left(), r.center().y, tr("settings.device.standby"), Some(tr("settings.device.standby_desc")), None);
    let key = Id::new("standby");
    let open = cx.menu_open(key);
    let resp = rows::select(ui, r.right(), r.center().y, key.with("btn"), standby_label(o.standby), open, None);
    if resp.clicked() || cx.take_demo("standby") {
        let entries = STANDBY_MENU
            .iter()
            .map(|v| {
                let e = Entry::plain(standby_label(*v), Action::SetStandby(*v));
                if *v == o.standby { e.meta(Meta::Check) } else { e }
            })
            .collect();
        cx.toggle_menu(key, Anchor::Rect(resp.rect), false, entries);
    }
    let r = rows_[1];
    rows::row_text(ui, r.left(), r.center().y, tr("settings.device.wake_threshold"), Some(tr("settings.device.wake_threshold_desc")), None);
    // The firmware has eight thresholds, 6 dB apart around the stock −71 dBFS: the slider moves by steps.
    let mut step = f32::from(o.threshold_step);
    let out = format!("{} dBFS", fmt::fr(f64::from(o.threshold_dbfs()), 0));
    let (lo, hi) = THRESHOLD_STEPS;
    let stock_step = f32::from(crate::model::settings::DeviceSettings::default().options.threshold_step);
    if rows::slider_out(ui, r.right(), r.center().y, Id::new("wk"), &mut step, (f32::from(lo), f32::from(hi), 1.0), stock_step, &out) {
        let mut n = s.clone();
        n.options.threshold_step = step.round() as i8;
        if n != s {
            cx.device.set_settings(n);
        }
    }

    p.section(ui, tr("settings.device.pod"));
    let r = p.list(ui, &[60.0])[0];
    rows::row_text(ui, r.left(), r.center().y, tr("settings.device.pod_double_press"), Some(tr("settings.device.pod_double_press_desc")), None);
    let mut on = o.double_tap;
    if switch::switch(ui, Pos2::new(r.right() - 40.0, r.center().y - 12.0), Id::new("cycle"), &mut on).changed() {
        cx.act(Action::SetDoubleTap(on));
    }
}

