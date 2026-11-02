//! Embedded assets: fonts, the product photo and the Lucide icon set. Everything is compiled into the
//! executable (`include_bytes!`), nothing is read from disk at run time.

pub mod app_icon;
pub mod fonts;
pub mod icons;
pub mod photo;
