//! egui's built-in fonts (Ubuntu Light, Hack, Noto Emoji, emoji-icon-font) are not listed: the app is built
//! without them (eframe without `default_fonts`).
//!
//! DRAFT: every legal text is a working draft that must be reviewed by a qualified lawyer before any release.

mod docs;
mod flow;
mod viewer;

use eframe::egui::{self, Context, Id};

use super::rows::Page;
use crate::i18n::tr;
use crate::theme;
use crate::views::Cx;
use docs::Doc;
use flow::{Flow, Item, Link};

const MAX_W: f32 = 640.0;

struct Third {
    name: &'static str,
    copy: &'static str,
    licence: &'static str,
    link: Option<Link>,
    pending: Option<&'static str>,
}

const fn doc(name: &'static str, copy: &'static str, licence: &'static str, d: Doc) -> Third {
    Third { name, copy, licence, link: Some(Link::Doc(d)), pending: None }
}

fn groups() -> [(&'static str, Vec<Third>); 3] {
    [
        (
            "legal.tp.g.ui",
            vec![
                doc("Inter 4.001", "© 2016 The Inter Project Authors", "SIL OFL 1.1", Doc::Inter),
                doc("Geist Mono 1.701", "© 2024 The Geist Project Authors", "SIL OFL 1.1", Doc::GeistMono),
                doc("Lucide", "© 2026 Lucide Icons and Contributors", "ISC", Doc::Lucide),
            ],
        ),
        (
            "legal.tp.g.app",
            vec![
                doc("egui / eframe 0.31", "© Emil Ernerfeldt", "legal.mit_or_apache", Doc::Egui),
                Third { name: "legal.rust", copy: "legal.rust.c", licence: "legal.rust.lic.short", link: None, pending: Some("legal.view.list") },
            ],
        ),
        (
            "legal.tp.g.fw",
            vec![
                Third { name: "legal.ti", copy: "legal.ti.c", licence: "legal.ti.lic", link: None, pending: Some("legal.tbc") },
                // No embedded text yet (licence to confirm): the project page.
                Third {
                    name: "SDCC",
                    copy: "legal.sdcc.c",
                    licence: "legal.sdcc.lic",
                    link: Some(Link::Web("https://sdcc.sourceforge.net")),
                    pending: None,
                },
            ],
        ),
    ]
}

/// A key, or a literal (names and licences that are not translated).
fn txt(s: &'static str) -> &'static str {
    if s.starts_with("legal.") {
        tr(s)
    } else {
        s
    }
}

fn doc_id() -> Id {
    Id::new("legal-doc")
}

fn open_doc(ctx: &Context) -> Option<Doc> {
    ctx.data(|d| d.get_temp::<Doc>(doc_id()))
}

pub fn close(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<Doc>(doc_id()));
}

/// Scroll identity of the pane: the page, or one per licence text (each opens at its top).
pub fn scroll_key(ctx: &Context) -> u64 {
    open_doc(ctx).map_or(0, Doc::key)
}

pub fn show(ui: &mut egui::Ui, _cx: &mut Cx, p: &mut Page) {
    let ctx = ui.ctx().clone();
    if let Some(d) = open_doc(&ctx) {
        if viewer::show(ui, p, d, MAX_W) {
            close(&ctx);
        }
        return;
    }
    p.head(ui, tr("settings.legal.title"), None);
    let t = theme::get(&ctx);
    let mut f = Flow { ui, t, left: p.left, width: p.width().min(MAX_W), y: p.y + 16.0, first: true, clicked: None };

    f.sec("legal.tm.title");
    f.p("legal.tm.p1");
    f.p("legal.tm.p2");
    f.p("legal.tm.p3");

    f.sec("legal.lic.title");
    f.licence_row(tr("legal.lic.line"), tr("legal.lic.p1"));
    f.p("legal.lic.p2");
    f.p("legal.lic.p3");

    f.sec("legal.risk.title");
    f.p("legal.risk.p");
    f.bullets();

    f.sec("legal.tp.title");
    for (gi, (gkey, items)) in groups().into_iter().enumerate() {
        f.sub(gkey);
        let rows: Vec<Item> = items
            .iter()
            .map(|it| Item {
                title: txt(it.name).to_owned(),
                sub: txt(it.copy).to_owned(),
                value: txt(it.licence).to_owned(),
                link: it.link,
                pending: it.pending,
            })
            .collect();
        f.list(&rows, &format!("legal-{gi}"));
    }
    p.y = f.y;
    match f.clicked {
        Some(Link::Doc(d)) => ctx.data_mut(|m| m.insert_temp(doc_id(), d)),
        Some(Link::Web(url)) => ctx.open_url(egui::OpenUrl::new_tab(url)),
        None => {}
    }
}
