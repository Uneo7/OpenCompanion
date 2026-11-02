use eframe::egui::{self, CursorIcon, Id, PointerButton, Pos2, Sense};

use super::{Change, Flags, Geo, Output, State};
use crate::model::biquad::{F_MAX, F_MIN};
use crate::model::eq::{Band, GAIN_LIMIT, Q_MAX, Q_MIN};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    FreqGain,
    Q,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Drag {
    index: usize,
    start: Pos2,
    freq: f64,
    gain: f64,
    q: f64,
    mode: Mode,
}

fn hit(geo: &Geo, bands: &[Band], flags: Flags, p: Pos2) -> Option<usize> {
    if flags.compare {
        None
    } else {
        geo.hit(bands, p)
    }
}

pub(crate) fn handle(
    ui: &mut egui::Ui,
    id: Id,
    geo: &Geo,
    bands: &mut [Band],
    selected: &mut usize,
    flags: Flags,
    st: &mut State,
) -> Output {
    let ctx = ui.ctx().clone();
    let mut out = Output::default();
    let resp = ui.interact(geo.canvas, id, Sense::click_and_drag());
    let (mods, pointer, pressed, down) = ctx.input(|i| {
        (i.modifiers, i.pointer.latest_pos(), i.pointer.primary_pressed(), i.pointer.primary_down())
    });
    let precise = mods.shift;
    let q_mode = mods.ctrl || mods.command;

    let inside = pointer.filter(|_| resp.hovered() || st.drag.is_some()).filter(|p| {
        p.x > geo.x0() && p.x < geo.x1() && p.y > geo.y0() && p.y < geo.y1()
    });
    st.hover = inside.map(|p| (geo.f_of(p.x).round(), (geo.d_of(p.y) * 10.0).round() / 10.0));
    if !resp.hovered() && st.drag.is_none() {
        st.hover = None;
        st.hover_node = None;
    }

    if pressed && resp.hovered() {
        if let Some(p) = pointer {
            if let Some(i) = hit(geo, bands, flags, p) {
                *selected = i;
                if mods.alt {
                    bands[i].on = !bands[i].on;
                    out.set(Change::Struct);
                } else {
                    let b = bands[i];
                    st.drag = Some(Drag {
                        index: i,
                        start: p,
                        freq: b.freq,
                        gain: b.gain,
                        q: b.q,
                        mode: if q_mode { Mode::Q } else { Mode::FreqGain },
                    });
                    out.set(Change::Select);
                }
            }
        }
    }

    if let (Some(mut d), Some(p)) = (st.drag, pointer) {
        if !down || d.index >= bands.len() {
            st.drag = None;
        } else {
            let k = if precise { 0.2 } else { 1.0 };
            let mode = if q_mode { Mode::Q } else { Mode::FreqGain };
            let b = &mut bands[d.index];
            if mode != d.mode {
                // Modifier changed mid-drag: continue from here.
                d = Drag { mode, start: p, freq: b.freq, gain: b.gain, q: b.q, ..d };
            }
            let before = *b;
            match mode {
                Mode::Q => {
                    let q = d.q * 2f64.powf(f64::from((d.start.y - p.y) * k / 60.0));
                    b.q = (q.clamp(Q_MIN, Q_MAX) * 100.0).round() / 100.0;
                }
                Mode::FreqGain => {
                    let span = geo.x1() - geo.x0();
                    let f = d.freq * (F_MAX / F_MIN).powf(f64::from((p.x - d.start.x) * k / span));
                    b.freq = f.clamp(F_MIN, F_MAX).round();
                    if b.kind.has_gain() {
                        let g = d.gain + (geo.d_of(p.y) - geo.d_of(d.start.y)) * f64::from(k);
                        b.gain = (g.clamp(-GAIN_LIMIT, GAIN_LIMIT) * 10.0).round() / 10.0;
                    }
                }
            }
            if *b != before {
                out.set(Change::Value);
            }
            st.drag = Some(d);
        }
    }

    if st.drag.is_none() {
        st.hover_node = pointer.filter(|_| resp.hovered()).and_then(|p| hit(geo, bands, flags, p));
    }

    if resp.clicked_by(PointerButton::Secondary) {
        if let Some(p) = pointer {
            if let Some(i) = hit(geo, bands, flags, p) {
                *selected = i;
                out.set(Change::Select);
                out.context = Some((i, p));
            }
        }
    }

    if let (Some(p), true) = (pointer, resp.hovered()) {
        let scroll = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(i) = hit(geo, bands, flags, p) {
                *selected = i;
                let k = if precise { 1.02 } else { 1.1 };
                let b = &mut bands[i];
                let q = if scroll > 0.0 { b.q * k } else { b.q / k };
                b.q = (q.clamp(Q_MIN, Q_MAX) * 100.0).round() / 100.0;
                out.set(Change::Value);
            }
        }
    }

    if resp.double_clicked() && !flags.compare && !flags.reference_only {
        if let Some(p) = pointer {
            match hit(geo, bands, flags, p) {
                Some(i) => {
                    if bands[i].kind.has_gain() {
                        bands[i].gain = 0.0;
                        out.set(Change::Value);
                    }
                }
                None => out.add = Some((geo.f_of(p.x).round(), (geo.d_of(p.y) * 10.0).round() / 10.0)),
            }
        }
    }

    if st.drag.is_some() {
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    } else if resp.hovered() {
        ctx.set_cursor_icon(if st.hover_node.is_some() {
            CursorIcon::Grab
        } else if flags.compare || flags.reference_only {
            CursorIcon::Default
        } else {
            CursorIcon::Crosshair
        });
    }
    out
}
