//! Inter 4.001 and Geist Mono 1.701, both SIL Open Font License 1.1.

pub const FACES: [(&str, &[u8]); 7] = [
    ("inter-light", include_bytes!("../../assets/fonts/Inter-Light.ttf")),
    ("inter-regular", include_bytes!("../../assets/fonts/Inter-Regular.ttf")),
    ("inter-medium", include_bytes!("../../assets/fonts/Inter-Medium.ttf")),
    ("inter-semibold", include_bytes!("../../assets/fonts/Inter-SemiBold.ttf")),
    ("mono-regular", include_bytes!("../../assets/fonts/GeistMono-Regular.ttf")),
    ("mono-medium", include_bytes!("../../assets/fonts/GeistMono-Medium.ttf")),
    ("mono-semibold", include_bytes!("../../assets/fonts/GeistMono-SemiBold.ttf")),
];

#[cfg(test)]
mod tests {
    use ab_glyph::{Font, FontRef};

    use super::FACES;

    const FR: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/fr.json"));
    const EN: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/en.json"));
    /// Symbols the code writes besides the strings (bullets, minus sign, −∞ dB, arrows, multiplication).
    const SYMBOLS: &str = "•−∞→×±…·«»";

    fn missing(face: &str, text: &str) -> String {
        let bytes = FACES.iter().find(|(n, _)| *n == face).expect("face").1;
        let font = FontRef::try_from_slice(bytes).expect("ttf");
        let mut out: Vec<char> = text.chars().filter(|c| !c.is_control() && font.glyph_id(*c).0 == 0).collect();
        out.sort_unstable();
        out.dedup();
        out.into_iter().collect()
    }

    /// egui's own fonts are not compiled in (no fallback): every character of the UI must be in the embedded
    /// faces.
    #[test]
    fn embedded_fonts_cover_every_ui_character() {
        let mut text = String::from(SYMBOLS);
        for json in [FR, EN] {
            let table: std::collections::HashMap<String, String> = serde_json::from_str(json).unwrap();
            table.values().for_each(|v| text.push_str(v));
        }
        for face in FACES.iter().map(|(n, _)| *n).filter(|n| n.starts_with("inter")) {
            assert_eq!(missing(face, &text), "", "{face}");
        }
        let mono: String = (' '..='~').chain("−∞·→…%°éèàç".chars()).collect();
        for face in FACES.iter().map(|(n, _)| *n).filter(|n| n.starts_with("mono")) {
            assert_eq!(missing(face, &mono), "", "{face}");
        }
    }
}
