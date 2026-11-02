use eframe::egui::{self, Id, Pos2};

use super::rows::{self, Page};
use crate::app::actions::Action;
use crate::i18n::tr;
use crate::views::Cx;
use crate::widgets::{fmt, switch, tooltip};

/// Room kept right of the sliders so they line up with the switch column (`.sw-gap` + the row gap).
const SW_GAP: f32 = 40.0 + 16.0;

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, p: &mut Page) {
    let name = cx.profile_name(cx.device.active_profile());
    p.head(ui, tr("settings.sound.title"), Some((tr("settings.sound.desc"), Some(&name))));
    let rows_ = p.list(ui, &[60.0; 2]);
    let mut s = cx.device.settings().clone();
    let before = s.clone();
    let locked = cx.device.profiles().get(cx.device.active_profile()).is_some_and(|p| p.locked);

    let r = rows_[0];
    rows::row_text(ui, r.left(), r.center().y, tr("settings.sound.loudness"), Some(tr("settings.sound.loudness_desc")), None);
    let at = Pos2::new(r.right() - 40.0, r.center().y - 12.0);
    let mut loud = s.loudness;
    if locked {
        tooltip::attach(&switch::switch_off(ui, at, Id::new("loud"), loud), tr("eq.locked"), None);
    } else if switch::switch(ui, at, Id::new("loud"), &mut loud).changed() {
        cx.act(Action::SetLoudness(loud));
    }

    let r = rows_[1];
    rows::row_text(ui, r.left(), r.center().y, tr("settings.sound.aux"), Some(tr("settings.sound.aux_desc")), None);
    let out = format!("{} dB", fmt::frs(f64::from(s.aux_gain), 1));
    if locked {
        rows::slider_off(ui, r.right() - SW_GAP, r.center().y, Id::new("ax"), s.aux_gain, (-6.0, 12.0, 0.5), &out, tr("eq.locked"));
    } else {
        rows::slider_out(ui, r.right() - SW_GAP, r.center().y, Id::new("ax"), &mut s.aux_gain, (-6.0, 12.0, 0.5), 0.0, &out);
    }

    if s != before {
        cx.device.set_settings(s);
    }
}
