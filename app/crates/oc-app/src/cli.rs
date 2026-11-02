//! Command line: `opencompanion [--sim] [--firmware <dir>] [--lang fr|en] [--screen home|equalizer|settings]
//! [--light] [--reduced-motion] [--shots <dir>]`. `OC_REDUCED_MOTION=1` also turns animations off.

use std::path::PathBuf;

use crate::app::state::Screen;
use crate::i18n::Lang;

#[derive(Debug, Clone)]
pub struct Options {
    pub screen: Screen,
    pub light: bool,
    pub reduced_motion: bool,
    pub shots: Option<PathBuf>,
    pub sim: bool,
    pub firmware: Option<PathBuf>,
    pub lang: Option<Lang>,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>, env_reduced: bool) -> Self {
        let args: Vec<String> = args.into_iter().collect();
        let value = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
        let flag = |f: &str| args.iter().any(|a| a == f);
        let screen = match value("--screen").as_deref() {
            // The French names are still accepted (older scripts).
            Some("equalizer" | "egaliseur") => Screen::Equalizer,
            Some("settings" | "reglages") => Screen::Settings,
            _ => Screen::Home,
        };
        Self {
            screen,
            light: flag("--light"),
            reduced_motion: flag("--reduced-motion") || env_reduced,
            shots: value("--shots").map(PathBuf::from),
            sim: flag("--sim"),
            firmware: value("--firmware").map(PathBuf::from),
            lang: value("--lang").as_deref().and_then(Lang::from_code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Options {
        Options::parse(s.split_whitespace().map(String::from), false)
    }

    #[test]
    fn defaults_and_flags() {
        let o = parse("oc");
        assert_eq!((o.screen, o.light, o.reduced_motion, o.shots.is_none()), (Screen::Home, false, false, true));
        let o = parse("oc --screen settings --light --reduced-motion --shots out");
        assert_eq!((o.screen, o.light, o.reduced_motion), (Screen::Settings, true, true));
        assert_eq!(o.shots, Some(PathBuf::from("out")));
        assert!(Options::parse(["oc".to_string()], true).reduced_motion);
        assert!(!o.sim && o.firmware.is_none() && o.lang.is_none());
        let o = parse("oc --sim --firmware fw --lang en");
        assert_eq!((o.sim, o.firmware, o.lang), (true, Some(PathBuf::from("fw")), Some(Lang::En)));
    }
}
