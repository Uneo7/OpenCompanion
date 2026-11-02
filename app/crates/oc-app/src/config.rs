use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::i18n::{self, Lang};

/// Settings › Preferences › Appearance. Auto (the default) follows the OS theme, live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    #[default]
    Auto,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// Unset = the OS language (French when the OS locale is French, English otherwise).
    #[serde(default)]
    pub lang: Option<Lang>,
    #[serde(default)]
    pub theme: ThemePref,
    /// Folder of the firmware files the installer uses (`oc_device::install::FirmwareSources::from_dir`);
    /// unset = the development tree.
    #[serde(default)]
    pub firmware_dir: Option<PathBuf>,
    /// The update offer the user hid on Home (its target versions): shown again for another version.
    #[serde(default)]
    pub dismissed_update: Option<String>,
    /// The app version whose download offer the user hid on Home.
    #[serde(default)]
    pub dismissed_app_update: Option<String>,
}

impl AppConfig {
    pub fn path() -> Option<PathBuf> {
        Some(oc_device::paths::config_file())
    }

    pub fn load() -> Self {
        Self::path().map(|p| Self::load_from(&p)).unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let p = Self::path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory on this system"))?;
        self.save_to(&p)
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(path, json)
    }

    pub fn apply(&self) {
        i18n::set_lang(self.lang.unwrap_or_else(os_lang));
    }
}

/// The OS UI language, mapped onto the app's languages.
pub fn os_lang() -> Lang {
    match sys_locale::get_locale() {
        Some(l) if l.to_ascii_lowercase().starts_with("fr") => Lang::Fr,
        _ => Lang::En,
    }
}

pub fn set_language(lang: Lang) -> io::Result<()> {
    i18n::set_lang(lang);
    update(|c| c.lang = Some(lang))
}

pub fn set_theme(theme: ThemePref) -> io::Result<()> {
    update(|c| c.theme = theme)
}

pub fn set_dismissed_app_update(version: &str) -> io::Result<()> {
    update(|c| c.dismissed_app_update = Some(version.to_owned()))
}

pub fn set_dismissed_update(key: &str) -> io::Result<()> {
    update(|c| c.dismissed_update = Some(key.to_owned()))
}

fn update(f: impl FnOnce(&mut AppConfig)) -> io::Result<()> {
    let mut c = AppConfig::load();
    f(&mut c);
    c.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("oc-app-config-{}", std::process::id()));
        let p = dir.join("sub").join("config.json");
        assert_eq!(AppConfig::load_from(&p), AppConfig::default());
        let c = AppConfig { lang: Some(Lang::En), theme: ThemePref::Light, firmware_dir: Some("fw".into()), dismissed_update: Some("DSP=0.2.6".into()), dismissed_app_update: Some("1.0.2".into()) };
        c.save_to(&p).unwrap();
        assert_eq!(AppConfig::load_from(&p), c);
        let json = std::fs::read_to_string(&p).unwrap();
        assert!(json.contains("\"lang\": \"en\"") && json.contains("\"theme\": \"light\""));
        std::fs::write(&p, "{}").unwrap();
        let d = AppConfig::load_from(&p);
        assert_eq!((d.lang, d.theme), (None, ThemePref::Auto));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
