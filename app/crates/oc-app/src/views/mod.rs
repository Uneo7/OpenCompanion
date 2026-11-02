pub mod actions;
pub mod chrome;
pub mod equalizer;
pub mod home;
pub mod install;
pub mod prefs;
pub mod settings;
pub mod state;

use eframe::egui::{self, Id, Rect, Vec2};

use crate::app::{actions::Action, state::Screen, state::UiState};
use crate::i18n::t;
use crate::model::{Device, Profile};
use crate::theme::{metrics, motion};
use crate::widgets::menu::{Anchor, Entry, Menu};

pub struct Cx<'a> {
    pub device: &'a mut dyn Device,
    pub ui: &'a mut UiState,
}

impl Cx<'_> {
    pub fn act(&mut self, a: Action) {
        self.ui.act(a);
    }

    pub fn toggle_menu(&mut self, key: Id, anchor: Anchor, align_right: bool, entries: Vec<Entry<Action>>) {
        if self.ui.menu.as_ref().is_some_and(|m| m.key == key) {
            self.ui.menu = None;
            return;
        }
        let serial = self.ui.next_serial();
        self.ui.menu = Some(Menu { key, anchor, align_right, entries, serial });
    }

    pub fn menu_open(&self, key: Id) -> bool {
        self.ui.menu.as_ref().is_some_and(|m| m.key == key)
    }

    pub fn take_demo(&mut self, name: &str) -> bool {
        if self.ui.demo.as_deref() == Some(name) {
            self.ui.demo = None;
            true
        } else {
            false
        }
    }

    /// Name of profile `i` as shown: a profile keeps its default (translated) name until the user renames it.
    pub fn profile_name(&self, i: usize) -> String {
        profile_name(&self.device.profiles()[i], i)
    }
}

use crate::model::profile::{DEFAULT_NAMES, LEGACY_DEFAULT_NAMES};

pub fn profile_name(p: &Profile, i: usize) -> String {
    if DEFAULT_NAMES.get(i).is_some_and(|d| *d == p.name) || LEGACY_DEFAULT_NAMES.get(i).is_some_and(|d| *d == p.name) {
        t(&format!("profile.default.p{}", i + 1))
    } else {
        p.name.clone()
    }
}

pub fn content_rect(main: Rect) -> Rect {
    let x = main.center().x - metrics::CONTENT_W / 2.0;
    Rect::from_min_max(
        egui::pos2(x, main.top() + metrics::S6),
        egui::pos2(x + metrics::CONTENT_W, main.bottom() - metrics::S6),
    )
}

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, main: Rect) {
    let ctx = ui.ctx().clone();
    let installing = install::visible(&ctx, cx);
    let key = if installing { 99 } else { cx.ui.screen as u64 };
    let k = motion::SPRING.at(motion::since(&ctx, Id::new("screen-enter"), key, 0.22));
    let content = content_rect(main).translate(Vec2::new(0.0, 4.0 * (1.0 - k)));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(main).id_salt(("screen", key)));
    child.set_opacity(k);
    if installing {
        install::show(&mut child, cx, content);
        return;
    }
    match cx.ui.screen {
        Screen::Home => home::show(&mut child, cx, content),
        Screen::Equalizer => equalizer::show(&mut child, cx, content),
        Screen::Settings => settings::show(&mut child, cx, content),
    }
}

pub fn reset(ctx: &egui::Context) {
    state::reset(ctx);
}
