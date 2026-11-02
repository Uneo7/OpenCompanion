//! UI strings: `oc-app/i18n/{fr,en}.json`, embedded, looked up by key (`area.element[.qualifier]`).
//!
//! Fallback: missing key in the current language → French → the key itself. Placeholders are `{name}`.
//! The current language is process-wide; [`crate::config`] persists the user's choice.

mod catalog;
#[cfg(test)]
mod coverage;

use std::sync::atomic::{AtomicUsize, Ordering};

pub use catalog::Lang;

static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn set_lang(l: Lang) {
    CURRENT.store(l.index(), Ordering::Relaxed);
}

pub fn lang() -> Lang {
    Lang::ALL[CURRENT.load(Ordering::Relaxed) % Lang::ALL.len()]
}

pub fn lookup(lang: Lang, key: &str) -> Option<&'static str> {
    catalog::tables()[lang.index()].get(key).map(String::as_str)
}

pub fn tr(key: &'static str) -> &'static str {
    lookup(lang(), key).or_else(|| lookup(Lang::Fr, key)).unwrap_or(key)
}

pub fn t(key: &str) -> String {
    lookup(lang(), key).or_else(|| lookup(Lang::Fr, key)).map_or_else(|| key.to_owned(), str::to_owned)
}

pub fn t_with(key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    args.iter().fold(t(key), |s, (name, v)| s.replace(&format!("{{{name}}}"), &v.to_string()))
}

pub fn missing_keys() -> Vec<(Lang, String)> {
    let mut out = Vec::new();
    for l in Lang::ALL {
        for other in Lang::ALL.into_iter().filter(|o| *o != l) {
            for k in catalog::tables()[other.index()].keys() {
                if lookup(l, k).is_none() {
                    out.push((l, k.clone()));
                }
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_fallback_and_placeholders() {
        assert_eq!(lookup(Lang::En, "app.nav.home"), Some("Home"));
        assert_eq!(lookup(Lang::Fr, "app.nav.home"), Some("Accueil"));
        assert_eq!(Lang::from_code("EN"), Some(Lang::En));
        assert_eq!(Lang::from_code("de"), None);
        // One test touches the global language, the others read tables only.
        set_lang(Lang::En);
        assert_eq!(tr("inst.step.dsp.critical"), "Audio processor · Sensitive step");
        assert_eq!(t("no.such.key"), "no.such.key");
        assert_eq!(t_with("eq.band", &[("n", &3)]), "Band 3");
        set_lang(Lang::Fr);
        assert_eq!(lang(), Lang::Fr);
        assert_eq!(t_with("eq.band", &[("n", &2)]), lookup(Lang::Fr, "eq.band").unwrap().replace("{n}", "2"));
    }

    #[test]
    fn both_tables_load() {
        for l in Lang::ALL {
            assert!(catalog::tables()[l.index()].len() > 100, "{} table", l.code());
        }
        // Reported, not enforced.
        let _ = missing_keys();
    }
}
