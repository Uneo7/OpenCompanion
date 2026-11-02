mod photo;
mod profiles;
mod app_update;
pub(crate) mod update;
mod volume;

use eframe::egui::{self, Pos2, Rect, Vec2};

use super::Cx;
use crate::assets::photo as asset;

const COL_W: f32 = 232.0;
const COL_GAP: f32 = 56.0;
const PROFILES_H: f32 = 16.0 + 8.0 + profiles::LIST_H;
const VOLUME_GAP: f32 = 44.0;

pub struct Layout {
    pub photo: Rect,
    pub column: Rect,
    pub profiles_top: f32,
    pub volume_top: f32,
}

pub fn layout(content: Rect) -> Layout {
    let column = Rect::from_min_max(Pos2::new(content.right() - COL_W, content.top()), content.max);
    let hero = Rect::from_min_max(content.min, Pos2::new(column.left() - COL_GAP, content.bottom()));
    let size = Vec2::new(asset::WIDTH, asset::WIDTH / asset::ASPECT);
    let photo = Rect::from_center_size(hero.center(), size);
    let total = PROFILES_H + VOLUME_GAP + volume::HEIGHT;
    let profiles_top = content.top() + (content.height() - total) / 2.0;
    Layout { photo, column, profiles_top, volume_top: profiles_top + PROFILES_H + VOLUME_GAP }
}

pub fn fade_center(main: Rect) -> Vec2 {
    let p = layout(super::content_rect(main)).photo.center();
    let win = crate::widgets::background::window_of(main);
    Vec2::new((p.x - win.left()) / win.width(), (p.y - win.top()) / win.height())
}

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, content: Rect) {
    let l = layout(content);
    photo::show(ui, l.photo);
    profiles::show(ui, cx, l.column, l.profiles_top);
    volume::show(ui, cx, l.column, l.volume_top);
    update::show(ui, cx, content);
}
