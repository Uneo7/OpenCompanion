//! The dialogs are modal and synchronous (macOS wants them on the main thread): views ask for one through
//! [`super::state::UiState::pick`], and the app opens it at the end of the frame with the window as its parent.

use std::path::Path;

use eframe::egui::Context;
use oc_device::install::{Chip, Flow};
use oc_device::params::file::{self as profile_file, EXTENSION};
use rfd::FileDialog;

use super::App;
use crate::i18n::{t_with, tr};
use crate::views::{self, actions::ViewAction, Cx};
use crate::widgets::dialog::{Body, Dialog};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Firmware(Chip),
    ExportProfile(usize),
    ImportProfile(usize),
}

const GENERATOR: &str = concat!("OpenCompanion ", env!("CARGO_PKG_VERSION"));

fn file_stem(name: &str) -> String {
    let s: String = name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches('.').to_owned();
    if s.is_empty() {
        "profile".into()
    } else {
        s
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

pub fn read_profile(path: &Path) -> Result<oc_device::params::Profile, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    profile_file::from_json(&json).map_err(|e| e.to_string())
}

impl App {
    fn file_dialog(frame: &eframe::Frame) -> FileDialog {
        FileDialog::new().set_parent(frame)
    }

    /// Profile export / import start in `<data>/profiles` (created on first use).
    fn profile_dialog(frame: &eframe::Frame) -> FileDialog {
        let dir = oc_device::paths::profiles_dir();
        let _ = std::fs::create_dir_all(&dir);
        Self::file_dialog(frame).set_directory(dir)
    }

    pub(super) fn pick(&mut self, ctx: &Context, frame: &eframe::Frame, pick: Pick) {
        let profiles = tr("profile.file.filter");
        match pick {
            Pick::Firmware(chip) => {
                // Only our `.ocfw` containers (checked before anything is written).
                let ext = oc_device::install::ocfw::EXTENSION;
                let Some(path) = Self::file_dialog(frame).add_filter(tr(chip.key()), &[ext]).pick_file() else { return };
                let mut cx = Cx { device: self.device.as_mut(), ui: &mut self.ui };
                views::install::open(ctx, &mut cx, Flow::File(chip, path));
            }
            Pick::ExportProfile(i) => {
                let name = views::profile_name(&self.device.profiles()[i], i);
                let Some(path) = Self::profile_dialog(frame)
                    .add_filter(profiles, &[EXTENSION])
                    .set_file_name(format!("{}.{EXTENSION}", file_stem(&name)))
                    .save_file()
                else {
                    return;
                };
                let json = profile_file::to_json(&self.device.profile_params(i), GENERATOR);
                let msg = match std::fs::write(&path, json) {
                    Ok(()) => t_with("profile.export.done", &[("file", &display_name(&path))]),
                    Err(e) => t_with("profile.export.error", &[("error", &e)]),
                };
                self.ui.toast.show(ctx, msg);
            }
            Pick::ImportProfile(i) => {
                let Some(path) = Self::profile_dialog(frame).add_filter(profiles, &[EXTENSION, "json"]).pick_file() else { return };
                match read_profile(&path) {
                    Ok(p) => {
                        let name = views::profile_name(&self.device.profiles()[i], i);
                        let serial = self.ui.next_serial();
                        self.ui.dialog = Some(Dialog {
                            title: t_with("profile.import.title", &[("name", &name)]),
                            body: Body::Text(t_with("profile.import.body", &[("file", &display_name(&path))])),
                            ok: tr("profile.import.ok").into(),
                            danger: false,
                            action: super::actions::Action::View(ViewAction::Import(i, Box::new(p))),
                            serial,
                        });
                    }
                    Err(e) => self.ui.toast.show(ctx, t_with("profile.import.error", &[("error", &e)])),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_portable() {
        assert_eq!(file_stem("Films: nuit?"), "Films_ nuit_");
        assert_eq!(file_stem(" Bureau. "), "Bureau");
        assert_eq!(file_stem("///"), "___");
        assert_eq!(file_stem(""), "profile");
    }

    #[test]
    fn exported_profiles_read_back() {
        let dir = std::env::temp_dir().join(format!("oc-app-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.ocprofile");
        let mut p = oc_device::params::Profile::defaults();
        p.set_name("Bureau");
        std::fs::write(&path, profile_file::to_json(&p, GENERATOR)).unwrap();
        assert_eq!(read_profile(&path), Ok(p));
        std::fs::write(&path, "{}").unwrap();
        assert!(read_profile(&path).is_err());
        assert!(read_profile(&dir.join("missing.ocprofile")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
