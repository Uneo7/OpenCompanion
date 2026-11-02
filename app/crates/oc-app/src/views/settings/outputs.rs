use eframe::egui::{self, Id, Pos2};

use super::rows::{self, Page};
use crate::app::actions::Action;
use crate::assets::icons;
use crate::i18n::tr;
use crate::model::{DeviceSettings, SubSource, Target};
use crate::views::Cx;
use crate::widgets::{
    fmt,
    segmented::{self, Segment},
};

const SUB_ROW_H: f32 = 96.0;

const TRIMS: [Target; 5] = [Target::InnerLeft, Target::InnerRight, Target::OuterLeft, Target::OuterRight, Target::Sub];

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, p: &mut Page) {
    p.head(ui, tr("settings.outputs.title"), None);
    p.section(ui, tr("settings.outputs.levels"));
    let rows_ = p.list(ui, &[60.0, 60.0, 60.0, 60.0, SUB_ROW_H]);
    let mut s = cx.device.settings().clone();
    let before = s.clone();
    // The trims are saved with the profile: disabled on the locked one. The bass module level stays editable.
    let locked = cx.device.profiles().get(cx.device.active_profile()).is_some_and(|p| p.locked);
    for (i, r) in rows_.iter().enumerate() {
        let icon = if i == 4 { icons::SUB } else { icons::SPEAKER };
        let icon_row = if i == 4 { egui::Rect::from_min_size(r.min, egui::Vec2::new(r.width(), 60.0)) } else { *r };
        let x = rows::row_icon(ui, icon_row, icon);
        if TRIMS[i] == Target::Sub {
            sub_row(ui, cx, *r, x, &mut s);
            continue;
        }
        rows::row_text(ui, x, r.center().y, tr(fmt::target_key(TRIMS[i])), None, None);
        let out = format!("{} dB", fmt::frs(f64::from(s.trims[i]), 1));
        if locked {
            rows::slider_off(ui, r.right(), r.center().y, Id::new(("trim", i)), s.trims[i], (-12.0, 6.0, 0.5), &out, tr("eq.locked"));
        } else {
            rows::slider_out(ui, r.right(), r.center().y, Id::new(("trim", i)), &mut s.trims[i], (-12.0, 6.0, 0.5), 0.0, &out);
        }
    }
    if s != before {
        cx.device.set_settings(s);
    }

    // Headphone EQ (device option, DSP profile protocol 4): flat, or the active profile's global EQ.
    if !cx.device.has_device_options() {
        return;
    }
    p.section(ui, tr("settings.outputs.headphones"));
    let r = p.list(ui, &[60.0])[0];
    let x = rows::row_icon(ui, r, icons::HEADPHONES);
    rows::row_text(ui, x, r.center().y, tr("settings.outputs.headphone_eq"), Some(tr("settings.outputs.headphone_eq_desc")), None);
    let on = cx.device.settings().options.headphone_eq;
    let segs = [Segment::text(tr("settings.outputs.flat")), Segment::text(tr("settings.outputs.profile"))];
    let size = segmented::size(ui.ctx(), &segs, false);
    let min = Pos2::new(r.right() - size.x, r.center().y - size.y / 2.0);
    if let Some(i) = segmented::segmented(ui, min, Id::new("hp-seg"), &segs, Some(usize::from(on)), false).0 {
        if (i == 1) != on {
            cx.act(Action::SetHeadphoneEq(i == 1));
        }
    }
}

fn sub_row(ui: &mut egui::Ui, cx: &mut Cx, r: egui::Rect, x: f32, s: &mut DeviceSettings) {
    let knob = s.sub_source == SubSource::Knob;
    let top_cy = r.top() + 30.0;
    let help = knob.then(|| tr("settings.outputs.sub.knob_help"));
    rows::row_text(ui, x, top_cy, tr(fmt::target_key(Target::Sub)), help, None);
    let segs = [Segment::text(tr("settings.outputs.sub.knob")), Segment::text(tr("settings.outputs.sub.app"))];
    let size = segmented::size(ui.ctx(), &segs, false);
    let min = Pos2::new(r.right() - size.x, top_cy - size.y / 2.0);
    if let Some(i) = segmented::segmented(ui, min, Id::new("sub-src"), &segs, Some(usize::from(!knob)), false).0 {
        s.sub_source = if i == 0 { SubSource::Knob } else { SubSource::App };
    }
    let cy = r.bottom() - 26.0;
    if knob {
        let db = cx.device.rear_knob_db();
        let out = db.map_or_else(|| "· dB".to_owned(), |v| format!("{} dB", fmt::frs(f64::from(v), 1)));
        rows::slider_ro(ui, r.right(), cy, Id::new("sub-knob"), db.unwrap_or(0.0), (-12.0, 12.0, 0.5), &out);
    } else {
        let out = format!("{} dB", fmt::frs(f64::from(s.sub_level), 1));
        rows::slider_out(ui, r.right(), cy, Id::new("sub-app"), &mut s.sub_level, (-12.0, 12.0, 0.5), 0.0, &out);
    }
}
