use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Vec2};

use super::menus;
use crate::assets::icons;
use crate::i18n::tr;
use crate::theme::{self, motion, typography::W};
use crate::views::Cx;
use crate::widgets::{button, button::IconButton, menu, menu::Anchor, text, text::st, tooltip};

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, head: Rect) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let title = text::line(ui.painter(), tr("eq.title"), &st::page(), t.text, head.left(), head.top() + 4.0, 32.0, Align::Min);

    let active = cx.device.active_profile();
    let p = cx.device.profiles()[active].clone();
    let name = cx.profile_name(active);
    let name_style = st::body(W::Medium);
    let name_w = text::width(&ctx, &name, &name_style);
    let w = 8.0 + 18.0 + 8.0 + name_w + 8.0 + 14.0 + 8.0;
    let pick = Rect::from_min_size(Pos2::new(title.right() + 12.0, head.center().y - 18.0), Vec2::new(w, 36.0));
    let key = Id::new("profile-menu");
    let resp = ui.interact(pick, key.with("btn"), Sense::click());
    let open = cx.menu_open(key);
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    if !open {
        tooltip::attach(&resp, tr("eq.tip.saved"), None);
    }
    let h = motion::hover(&ctx, key, resp.hovered() || open);
    ui.painter().rect_filled(pick, 8.0, theme::alpha(t.field, h));
    let h1 = motion::color(&ctx, key.with("h1"), theme::rgb(p.hue().a), 0.3, motion::EASE);
    let sw = Rect::from_min_size(Pos2::new(pick.left() + 8.0, pick.center().y - 1.5), Vec2::new(18.0, 3.0));
    ui.painter().rect_filled(sw, 2.0, h1);
    let nr = text::line(ui.painter(), &name, &name_style, t.text, sw.right() + 8.0, pick.top(), 36.0, Align::Min);
    let x = nr.right();
    menu::chevron(ui, Pos2::new(x + 8.0 + 7.0, pick.center().y), key.with("chev"), open);
    if resp.clicked() || cx.take_demo("profile-menu") {
        let entries = menus::profile_menu(cx, active);
        cx.toggle_menu(key, Anchor::Rect(pick), false, entries);
    }

    // "Bose® d'origine": read-only. A lock and a short note instead of the pencil and Enregistrer.
    if p.locked {
        let lc = Pos2::new(pick.right() + 10.0 + 7.0, pick.center().y);
        icons::LOCK.paint(ui.painter(), lc, 14.0, t.muted);
        text::line(ui.painter(), tr("eq.locked"), &st::inter_(12.0, W::Regular), t.muted, lc.x + 7.0 + 6.0, pick.top(), 36.0, Align::Min);
        duplicate_button(ui, cx, active, head.right(), head.center().y);
        return;
    }

    let pencil = Rect::from_min_size(Pos2::new(head.right() - 36.0, head.center().y - 18.0), Vec2::splat(36.0));
    duplicate_button(ui, cx, active, pencil.left() - 8.0, head.center().y);
    let ekey = Id::new("edit-menu");
    let eopen = cx.menu_open(ekey);
    let eresp = IconButton::new(icons::PENCIL).open(eopen).tip(tr("profile.edit"), None).show(ui, pencil, ekey.with("btn"));
    if eresp.clicked() || cx.take_demo("edit-menu") {
        let entries = menus::edit_menu(cx, active);
        cx.toggle_menu(ekey, Anchor::Rect(pencil), true, entries);
    }

}

/// "Dupliquer" on every profile, right-aligned at `right`: opens the target picker ("Dupliquer dans…").
fn duplicate_button(ui: &mut egui::Ui, cx: &mut Cx, active: usize, right: f32, cy: f32) {
    let dup = button::Button::new(tr("profile.duplicate")).icon(icons::COPY).small().kind(button::Kind::Quiet);
    let size = dup.size(ui.ctx());
    let key = Id::new("dup-menu");
    let resp = dup.show(ui, Pos2::new(right - size.x, cy - size.y / 2.0), key.with("btn"));
    if resp.clicked() || cx.take_demo("dup-menu") {
        let entries = menus::duplicate_entries(cx, active);
        cx.toggle_menu(key, Anchor::Rect(resp.rect), false, entries);
    }
}
