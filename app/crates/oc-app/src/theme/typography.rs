//! Every face is its own egui family (egui has no font weights).

use std::sync::Arc;

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily, FontId};

use crate::assets::fonts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W {
    Regular,
    Medium,
    SemiBold,
}

const fn inter_face(w: W) -> &'static str {
    match w {

        W::Regular => "inter-regular",
        W::Medium => "inter-medium",
        W::SemiBold => "inter-semibold",
    }
}

const fn mono_face(w: W) -> &'static str {
    match w {
        W::Regular => "mono-regular",
        W::Medium => "mono-medium",
        W::SemiBold => "mono-semibold",
    }
}

pub fn inter(size: f32, w: W) -> FontId {
    FontId::new(size, FontFamily::Name(inter_face(w).into()))
}

pub fn mono(size: f32, w: W) -> FontId {
    FontId::new(size, FontFamily::Name(mono_face(w).into()))
}

/// Registers all embedded faces. egui's own fonts (Ubuntu Light, Hack, emoji) are not compiled in (eframe
/// without `default_fonts`): the embedded faces cover every character of the UI (`assets::fonts` tests).
pub fn install(ctx: &Context) {
    let mut defs = FontDefinitions::empty();
    for (name, bytes) in fonts::FACES {
        defs.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        defs.families.insert(FontFamily::Name(name.into()), vec![name.to_owned()]);
    }
    defs.families.insert(FontFamily::Proportional, vec!["inter-regular".to_owned()]);
    defs.families.insert(FontFamily::Monospace, vec!["mono-regular".to_owned()]);
    ctx.set_fonts(defs);
}
