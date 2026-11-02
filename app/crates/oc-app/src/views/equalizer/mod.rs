mod band_panel;
mod header;
pub(crate) mod menus;
mod toolbar;

use eframe::egui::{self, Id, Key, Pos2, Rect, Vec2};

use super::Cx;
use crate::app::actions::Action;
use crate::assets::icons;
use crate::i18n::tr;
use crate::model::eq::MAX_BANDS;
use crate::model::{reference, Target};
use crate::widgets::button::IconButton;
use crate::widgets::eq_graph::{self, Flags, Reference};
use crate::widgets::menu::Anchor;

/// Vertical layout (from the content top, 16 px gaps): head 40 (+ 8), bar 36, [driver bar 36], graph 318
/// (262 in Advanced), band panel.
struct Layout {
    head: Rect,
    bar: Rect,
    drivers: Option<Rect>,
    graph: Rect,
    panel: Rect,
}

fn layout(content: Rect, advanced: bool) -> Layout {
    let row = |y: f32, h: f32| Rect::from_min_size(Pos2::new(content.left(), y), Vec2::new(content.width(), h));
    let head = row(content.top(), 40.0);
    let bar = row(head.bottom() + 8.0 + 16.0, 36.0);
    let drivers = advanced.then(|| row(bar.bottom() + 16.0, 36.0));
    let graph = row(drivers.unwrap_or(bar).bottom() + 16.0, if advanced { 262.0 } else { 318.0 });
    let panel = Rect::from_min_max(Pos2::new(content.left(), graph.bottom() + 16.0), Pos2::new(content.right(), content.bottom() + 32.0));
    Layout { head, bar, drivers, graph, panel }
}

fn target_index(t: Target) -> u64 {
    Target::ALL.iter().position(|x| *x == t).unwrap_or(0) as u64
}

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, content: Rect) {
    let active = cx.device.active_profile();
    // "Bose® d'origine" is read-only and always shown per output (the Bose response only makes sense there).
    let locked = cx.device.profiles()[active].locked;
    // The target follows the profile's EQ mode: an output in Advanced (and on the locked profile), else the global
    // EQ ("Simple").
    let per_output = locked || cx.ui.eq.draft.advanced;
    if per_output != cx.ui.eq.target.is_advanced() {
        cx.act(Action::SetTarget(if per_output { cx.ui.eq.driver } else { Target::Global }));
    }
    // The locked profile shows its output picker in the bar itself (no Simple / Advanced), so no second row.
    let advanced = !locked && cx.ui.eq.draft.advanced;
    let l = layout(content, advanced);

    header::show(ui, cx, l.head);
    toolbar::show(ui, cx, l.bar, l.drivers);

    let eq = &mut cx.ui.eq;
    let target = eq.target;
    let base: Vec<_> = if target == Target::Global { Vec::new() } else { eq.draft.get(Target::Global).clone() };
    let count = eq.draft.get(target).len();
    let flags = Flags {
        compare: eq.compare && !locked,
        bypass: eq.bypass && !locked,
        can_add: count < MAX_BANDS && !locked,
        morph_key: target_index(target) * 8 + active as u64,
        reference_only: locked,
    };
    // The real response of the Bose sound every profile plays on (precomputed, `model::reference`).
    // Comparing plays the built-in Bose profile (loudness on); the bypass turns the loudness off.
    let loudness = eq.compare || (cx.device.settings().loudness && !eq.bypass);
    let curves = reference::curves(loudness);
    let label = tr("eq.ref.bose");
    // Never in "Simple": the Bose response is per output.
    let refx = (target != Target::Global).then(|| Reference { curve: curves.of(target), label, tip: tr("eq.ref.tip") });
    // The locked profile shows no band (and none can be added or edited).
    let mut none = Vec::new();
    let (bands, base) = if locked { (&mut none, Vec::new()) } else { (eq.draft.get_mut(target), base) };
    let out = eq_graph::eq_graph(ui, l.graph, Id::new("eq-graph"), bands, &base, &mut eq.selected, flags, refx);
    if let Some(change) = out.change {
        cx.act(Action::Edited(change));
    }
    if let Some(at) = out.add {
        cx.act(Action::AddBand(Some(at)));
    }
    if let Some((i, p)) = out.context {
        let b = cx.ui.eq.draft.get(target)[i];
        let entries = menus::band_menu(i, &b);
        cx.ui.menu = None;
        cx.toggle_menu(Id::new(("band-menu", i)), Anchor::Point(p), false, entries);
    }

    let help = Rect::from_min_size(Pos2::new(l.graph.right() - 30.0, l.graph.top()), Vec2::splat(30.0));
    let key = Id::new("help-pop");
    let open = cx.menu_open(key);
    let tip = tr("eq.tip.gestures");
    if IconButton::new(icons::HELP).open(open).tip(tip, None).show(ui, help, key.with("btn")).clicked() || cx.take_demo("help") {
        cx.toggle_menu(key, Anchor::Rect(help), true, menus::help());
    }

    if !locked {
        band_panel::show(ui, cx, l.panel);
    }

    let typing = ui.ctx().wants_keyboard_input();
    if !locked && !typing && cx.ui.dialog.is_none() && ui.ctx().input(|i| i.key_pressed(Key::Delete)) {
        cx.act(Action::DeleteBand);
    }
}
