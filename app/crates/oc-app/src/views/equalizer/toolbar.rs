use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Vec2};

use super::menus;
use crate::app::actions::Action;
use crate::assets::icons;
use crate::i18n::{t_with, tr};
use crate::model::{eq::MAX_BANDS, Target};
use crate::theme::{self, motion, typography::W};
use crate::views::Cx;
use crate::widgets::{
    button::IconButton,
    menu::Anchor,
    segmented::{self, Segment},
    switch, text,
    text::st,
    tooltip,
};

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, bar: Rect, drivers: Option<Rect>) {
    let target = cx.ui.eq.target;
    // The locked profile: only the output picker (no Simple / Advanced, no band tools, no link).
    let locked = cx.device.profiles()[cx.device.active_profile()].locked;
    let drivers = if locked { Some(bar) } else { drivers };
    if !locked {
        tools(ui, cx, bar, target);
    }
    drivers_row(ui, cx, drivers, target, !locked);
}

fn tools(ui: &mut egui::Ui, cx: &mut Cx, bar: Rect, target: Target) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);

    // EQ mode of the profile: Simple (one global EQ) or Advanced (one EQ per output). Simple → Advanced keeps the
    // sound; Advanced → Simple asks first (the per-output settings are replaced by a flat global EQ).
    let segs = [
        Segment::icon(icons::SPEAKERS, tr("eq.mode.simple")).tip(tr("eq.tip.mode.simple")),
        Segment::icon(icons::SLIDERS, tr("eq.mode.advanced")).tip(tr("eq.tip.mode.advanced")),
    ];
    let advanced = cx.ui.eq.draft.advanced;
    let (clicked, _) = segmented::segmented(ui, bar.min, Id::new("eq-target"), &segs, Some(usize::from(advanced)), true);
    match clicked {
        Some(1) if !advanced => cx.act(Action::SetAdvanced(true)),
        Some(0) if advanced => cx.act(Action::AskSimple),
        _ => {}
    }

    let more = Rect::from_min_size(Pos2::new(bar.right() - 36.0, bar.top()), Vec2::splat(36.0));
    let key = Id::new("more-menu");
    let open = cx.menu_open(key);
    let resp = IconButton::new(icons::MORE).open(open).tip(tr("eq.tip.more"), None).show(ui, more, key.with("btn"));
    if resp.clicked() || cx.take_demo("more-menu") {
        let entries = menus::more_menu(&cx.ui.eq);
        cx.toggle_menu(key, Anchor::Rect(more), true, entries);
    }
    let add = more.translate(Vec2::new(-38.0, 0.0));
    let n = cx.ui.eq.draft.get(target).len();
    let full = n >= MAX_BANDS;
    let compare = cx.ui.eq.compare;
    let max_tip = t_with("eq.max", &[("n", &MAX_BANDS)]);
    let add_tip = if full { max_tip.as_str() } else { tr("eq.tip.add_band") };
    let r = IconButton::new(icons::PLUS).enabled(!full && !compare).tip(add_tip, None).show(ui, add, Id::new("eq-add"));
    if full {
        tooltip::attach(&r, add_tip, None);
    }
    if r.clicked() {
        cx.act(Action::AddBand(None));
    }
    if !compare {
        let count = format!("{n} / {MAX_BANDS}");
        text::line(ui.painter(), &count, &st::mono_(12.0, W::Regular), t.faint, add.left() - 6.0, bar.top(), 36.0, Align::Max);
    }

}

fn drivers_row(ui: &mut egui::Ui, cx: &mut Cx, drivers: Option<Rect>, target: Target, link: bool) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    if let Some(row) = drivers {
        let k = motion::SPRING.at(motion::since(&ctx, Id::new("eq-adv-enter"), 1, 0.26));
        let row = row.translate(Vec2::new(0.0, -6.0 * (1.0 - k)));
        let segs = [
            Segment::text(tr("eq.inner_l")).tip(tr("eq.tip.inner_l")),
            Segment::text(tr("eq.inner_r")).tip(tr("eq.tip.inner_r")),
            Segment::text(tr("eq.outer_l")).tip(tr("eq.tip.outer_l")),
            Segment::text(tr("eq.outer_r")).tip(tr("eq.tip.outer_r")),
            Segment::text(tr("eq.sub")).tip(tr("eq.tip.sub")),
        ];
        let cur = Target::ADVANCED.iter().position(|d| *d == target);
        let (clicked, seg_rect) = segmented::segmented(ui, row.min, Id::new("eq-driver"), &segs, cur, false);
        if let Some(i) = clicked {
            cx.act(Action::SetTarget(Target::ADVANCED[i]));
        }
        if !link {
            return;
        }
        let x = seg_rect.right() + 12.0;
        let label = tr("eq.aria.link");
        let style = st::sec(W::Regular);
        let lw = text::width(&ctx, label, &style);
        let area = Rect::from_min_size(Pos2::new(x, row.top()), Vec2::new(40.0 + 8.0 + lw, row.height()));
        let resp = ui.interact(area, Id::new("eq-link"), Sense::click());
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        tooltip::attach(&resp, tr("eq.tip.link"), None);
        if resp.clicked() {
            cx.act(Action::SetLink(!cx.ui.eq.link));
        }
        let sw = Rect::from_min_size(Pos2::new(x, row.center().y - 12.0), switch::SIZE);
        switch::paint_switch(ui.painter(), &ctx, sw, Id::new("eq-link-sw"), cx.ui.eq.link, resp.hovered(), &t);
        text::line(ui.painter(), label, &style, t.text, x + 48.0, row.top(), row.height(), Align::Min);
    } else {
        ctx.data_mut(|d| d.remove::<(u64, f64)>(Id::new("eq-adv-enter")));
    }
}
