use eframe::egui::{self, Id, Pos2};

use super::rows::{self, Page};
use crate::assets::icons;
use crate::i18n::{self, tr, Lang};
use crate::views::{prefs, state::ThemePref, Cx};
use crate::widgets::segmented::{self, Segment};

fn seg_right(ui: &mut egui::Ui, right: f32, cy: f32, id: Id, segs: &[Segment], sel: usize) -> Option<usize> {
    let size = segmented::size(ui.ctx(), segs, false);
    segmented::segmented(ui, Pos2::new(right - size.x, cy - size.y / 2.0), id, segs, Some(sel), false).0
}

const THEMES: [ThemePref; 3] = [ThemePref::Auto, ThemePref::Dark, ThemePref::Light];

pub fn show(ui: &mut egui::Ui, _cx: &mut Cx, p: &mut Page) {
    let ctx = ui.ctx().clone();
    p.head(ui, tr("settings.prefs.title"), None);

    p.section(ui, tr("settings.prefs.display"));
    let r = p.list(ui, &[60.0])[0];
    rows::row_text(ui, r.left(), r.center().y, tr("settings.prefs.appearance"), None, None);
    let segs = [
        Segment::icon(icons::AUTO, tr("settings.prefs.theme.auto")),
        Segment::icon(icons::MOON, tr("settings.prefs.theme.dark")),
        Segment::icon(icons::SUN, tr("settings.prefs.theme.light")),
    ];
    let cur = THEMES.iter().position(|x| *x == prefs::theme(&ctx)).unwrap_or(0);
    if let Some(i) = seg_right(ui, r.right(), r.center().y, Id::new("theme"), &segs, cur) {
        prefs::set_theme(&ctx, THEMES[i]);
    }

    p.section(ui, tr("settings.prefs.language"));
    let r = p.list(ui, &[60.0])[0];
    rows::row_text(ui, r.left(), r.center().y, tr("settings.prefs.language"), Some(tr("settings.prefs.language_desc")), None);
    // Each language is named in itself.
    let segs = [Segment::text("Français"), Segment::text("English")];
    let cur = Lang::ALL.iter().position(|l| *l == i18n::lang()).unwrap_or(0);
    if let Some(i) = seg_right(ui, r.right(), r.center().y, Id::new("lang"), &segs, cur) {
        prefs::set_lang(Lang::ALL[i]);
    }
}
