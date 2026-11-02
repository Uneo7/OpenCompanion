//! Licence texts embedded in the app (`opencompanion/licenses/`, `LICENSE`: GPL-3.0-or-later): every
//! "voir le texte" opens one of them inside the app, offline. The SIL OFL requires the font licence to travel
//! with the bundled fonts (Inter, Geist Mono).

macro_rules! root {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../", $path))
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doc {
    Gpl,
    Inter,
    GeistMono,
    Lucide,
    Egui,
}

impl Doc {
    /// Title of the viewer (names and licence names are not translated).
    pub fn title(self) -> &'static str {
        match self {
            Doc::Gpl => "OpenCompanion · GPL-3.0-or-later",
            Doc::Inter => "Inter 4.001 · SIL OFL 1.1",
            Doc::GeistMono => "Geist Mono 1.701 · SIL OFL 1.1",
            Doc::Lucide => "Lucide · ISC",
            Doc::Egui => "egui / eframe · MIT",
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            Doc::Gpl => root!("LICENSE"),
            Doc::Inter => root!("licenses/OFL-1.1-Inter.txt"),
            Doc::GeistMono => root!("licenses/OFL-1.1-GeistMono.txt"),
            Doc::Lucide => root!("licenses/ISC-Lucide.txt"),
            Doc::Egui => root!("licenses/MIT-egui.txt"),
        }
    }

    /// Scroll identity of the viewer (0 is the Legal notices page).
    pub fn key(self) -> u64 {
        self as u64 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_text_is_embedded() {
        for d in [Doc::Gpl, Doc::Inter, Doc::GeistMono, Doc::Lucide, Doc::Egui] {
            assert!(d.text().len() > 500, "{d:?}");
        }
        assert!(Doc::Inter.text().contains("SIL OPEN FONT LICENSE Version 1.1"));
        assert!(Doc::Gpl.text().contains("GNU GENERAL PUBLIC LICENSE") && Doc::Gpl.text().contains("Version 3"));
        assert!(Doc::Lucide.text().starts_with("ISC License"));
    }
}
