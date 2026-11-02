use eframe::egui::{self, Align, Color32, Id, Pos2, Rect, Vec2};

use crate::app::actions::Action;
use crate::assets::icons;
use crate::i18n::tr;
use crate::model::biquad::{F_MAX, F_MIN};
use crate::model::eq::{Band, BandType, GAIN_LIMIT};
use crate::theme::{self, motion};
use crate::views::Cx;
use crate::widgets::{button::IconButton, eq_graph::Change, fmt, scrubber, slider, text, text::st};

#[derive(Clone, Copy)]
enum Field {
    Freq,
    Gain,
    Q,
}

/// `.scrub` block: padding 6 / 10 / 8; caption 16, gap 2, value 36, gap 2, slider (2 + 32).
const SCRUB_H: f32 = 6.0 + 16.0 + 2.0 + 36.0 + 2.0 + 34.0 + 8.0;
const ACTS_W: f32 = 36.0 + 4.0 + 36.0;
const COL_GAP: f32 = 32.0;

fn pos(field: Field, b: &Band) -> f32 {
    match field {
        Field::Freq => ((b.freq / F_MIN).ln() / (F_MAX / F_MIN).ln()) as f32,
        Field::Gain => ((if b.kind.has_gain() { b.gain } else { 0.0 }.clamp(-GAIN_LIMIT, GAIN_LIMIT) + GAIN_LIMIT) / (2.0 * GAIN_LIMIT)) as f32,
        Field::Q => ((b.q / 0.1).ln() / 120f64.ln()) as f32,
    }
}

/// Value a double-click restores: gain 0 dB, the type's default Q, the type's default frequency (the one a new
/// band of that type gets).
fn default_value(field: Field, b: &Band) -> f64 {
    match field {
        Field::Freq => match b.kind {
            BandType::HighPass => 30.0,
            BandType::LowPass => 16_000.0,
            _ => 1000.0,
        },
        Field::Gain => 0.0,
        Field::Q => {
            if b.kind == BandType::Peak {
                1.0
            } else {
                0.71
            }
        }
    }
}

/// Value of a field at slider position `p` (the mockup's mapping, 1000 steps).
fn at(field: Field, p: f32) -> f64 {
    let p = f64::from(p);
    match field {
        Field::Freq => F_MIN * (F_MAX / F_MIN).powf(p),
        Field::Gain => p * 2.0 * GAIN_LIMIT - GAIN_LIMIT,
        Field::Q => 0.1 * 120f64.powf(p),
    }
}

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, panel: Rect, top: f32, sel: usize, band: Band, hue: Color32) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let col_w = (panel.width() - ACTS_W - 3.0 * COL_GAP) / 3.0;
    let off = motion::toggle(&ctx, Id::new("bx-off"), !band.on, 0.2, motion::EASE);
    let has_gain = band.kind.has_gain();
    let fields = [(Field::Freq, "eq.f"), (Field::Gain, "eq.g"), (Field::Q, "eq.q")];
    for (i, (field, key)) in fields.into_iter().enumerate() {
        let x = panel.left() + i as f32 * (col_w + COL_GAP);
        let block = Rect::from_min_size(Pos2::new(x - 10.0, top), Vec2::new(col_w + 20.0, SCRUB_H));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(block.expand(4.0)));
        child.set_opacity(1.0 - 0.55 * off);
        let (display, unit, step, enabled) = match field {
            Field::Freq => {
                let (v, u) = fmt::freq_parts(band.freq);
                (v, u, band.freq * 0.015, true)
            }
            Field::Gain if has_gain => (fmt::frs(band.gain, 1), "dB", 0.1, true),
            Field::Gain => ("-".to_owned(), "", 0.1, false),
            Field::Q => (fmt::fr(band.q, 2), "Q", band.q * 0.03, true),
        };
        let value = match field {
            Field::Freq => band.freq,
            Field::Gain => band.gain,
            Field::Q => band.q,
        };
        let cap_top = top + 6.0;
        text::line(child.painter(), tr(key), &st::sec_t(), t.muted, x, cap_top, 16.0, Align::Min);
        let vbox = Rect::from_min_size(Pos2::new(x, cap_top + 16.0 + 2.0), Vec2::new(col_w, 36.0));
        let tip = crate::i18n::t(&format!("{key}.tip"));
        let mut new = scrubber::scrubber(
            &mut child,
            Id::new(("bx-val", i)),
            scrubber::Scrubber { field: block, value_box: vbox, value, display: &display, unit, step, enabled, color: hue, tip: &tip, default: Some(default_value(field, &band)) },
        );
        let srect = Rect::from_min_size(Pos2::new(x, vbox.bottom() + 2.0 + 2.0), Vec2::new(col_w, 32.0));
        let mut p = pos(field, &band);
        let mut reset_band = band;
        match field {
            Field::Freq => reset_band.freq = default_value(field, &band),
            Field::Gain => reset_band.gain = 0.0,
            Field::Q => reset_band.q = default_value(field, &band),
        }
        let opts = slider::Opts { accent: Some(hue), ring: true, disabled: !enabled, reset: Some(pos(field, &reset_band)), read_only: false };
        if slider::slider_ex(&mut child, srect, Id::new(("bx-range", i)), &mut p, (0.0, 1.0, 0.001), slider::Look::Thin, opts).changed() {
            new = Some(at(field, p));
        }
        if let Some(v) = new {
            apply(cx, sel, field, v);
        }
    }

    // On/off and delete, bottom-aligned with the values (8 px above the block's end).
    let by = top + SCRUB_H - 8.0 - 36.0 - 8.0;
    let on_rect = Rect::from_min_size(Pos2::new(panel.right() - ACTS_W, by), Vec2::splat(36.0));
    let del_rect = on_rect.translate(Vec2::new(40.0, 0.0));
    let on_tip = tr(if band.on { "eq.off.tip" } else { "eq.on.tip" });
    if IconButton::new(icons::BYPASS).round().on(band.on).tip(on_tip, None).show(ui, on_rect, Id::new("bx-on")).clicked() {
        cx.act(Action::ToggleBand(sel));
    }
    let del = IconButton::new(icons::TRASH).danger().tip(tr("eq.del.tip"), Some(tr("kbd.delete")));
    if del.show(ui, del_rect, Id::new("bx-del")).clicked() {
        cx.act(Action::DeleteBand);
    }
}

/// Writes a scrubbed, slid or typed value into the draft (rounded and clamped like the speaker expects).
fn apply(cx: &mut Cx, sel: usize, field: Field, v: f64) {
    let target = cx.ui.eq.target;
    if let Some(b) = cx.ui.eq.draft.get_mut(target).get_mut(sel) {
        let before = *b;
        match field {
            Field::Freq => b.set_freq(v),
            Field::Gain => b.set_gain(v),
            Field::Q => b.set_q(v),
        }
        if *b != before {
            cx.act(Action::Edited(Change::Value));
        }
    }
}
