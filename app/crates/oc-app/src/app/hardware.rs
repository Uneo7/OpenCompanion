use std::path::Path;

use eframe::egui::Context;
use oc_device::install::{Flow, FirmwareSources, Phase};

use super::actions::{Action, InstallKind};
use super::state::Prompt;
use super::App;
use crate::i18n::tr;
use crate::model::Connection;

/// The installer's inputs (default builds): `dir` (`--firmware`, or saved), else the development tree this app
/// was built from (latest builds).
#[cfg(not(feature = "release-updates"))]
pub fn sources(dir: Option<&Path>) -> FirmwareSources {
    match dir {
        Some(d) => FirmwareSources::from_dir(d),
        None => FirmwareSources::dev_tree(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))),
    }
}

/// The installer's inputs (feature `release-updates`): the newest firmware release downloaded from GitHub, never
/// the development tree nor `--firmware`. None yet: an empty folder, until the update check downloads one.
#[cfg(feature = "release-updates")]
pub fn sources(_dir: Option<&Path>) -> FirmwareSources {
    let root = oc_device::paths::firmware_dir();
    match oc_device::install::release::store::latest(&root) {
        Some((_, dir, _)) => FirmwareSources::from_dir(&dir),
        None => FirmwareSources::from_dir(&root.join("none")),
    }
}

impl App {
    pub(super) fn watch_hardware(&mut self, ctx: &Context) {
        if let Some(n) = self.device.take_notice() {
            self.ui.toast.show(ctx, n);
        }
        while let Some(out) = self.device.take_console_output() {
            self.ui.settings.console += &format!("{out}\n");
        }
        if self.device.take_profile_reloaded() && !self.ui.eq.dirty {
            let bands = self.device.profiles()[self.device.active_profile()].bands.clone();
            if self.ui.eq.load(bands) {
                self.ui.toast.show(ctx, crate::i18n::tr("eq.boost.scaled"));
            }
        }
        self.offer_install(ctx);
        self.plan_prompt(ctx);
        let on_firmware = self.ui.screen == super::state::Screen::Settings && self.ui.settings.pane == super::state::Pane::Firmware;
        if on_firmware && !self.ui.hw.on_firmware {
            self.ui.hw.check_pending = true;
        }
        self.ui.hw.on_firmware = on_firmware;
        // Launch and every regained focus refresh the update check too.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.ui.hw.focused {
            self.ui.hw.check_pending = true;
        }
        self.ui.hw.focused = focused;
        // The update check waits for the backup check of a new connection (one job on the speaker at a time).
        let ready = self.device.connection() == Connection::Ready;
        if self.ui.hw.check_pending && ready && self.device.installer().is_some_and(|i| i.check_updates()) {
            self.ui.hw.check_pending = false;
        }
    }

    fn offer_install(&mut self, ctx: &Context) {
        let c = self.device.connection();
        if self.ui.hw.connection.as_ref() == Some(&c) || self.ui.dialog.is_some() {
            return;
        }
        self.ui.hw.connection = Some(c.clone());
        oc_device::install::log_file::append(&[format!("speaker: {c:?}")]);
        match c {
            // Bose firmware (or a mix): the installer view opens by itself as the onboarding (views::install).
            // Every connection of a ready speaker compares its firmware with the app's.
            Connection::Ready => self.ui.hw.check_pending = true,
            Connection::Recovery => self.ui.toast.show(ctx, format!("{} · {}", tr("inst.err.title"), tr("inst.err.guide"))),
            Connection::Inaccessible(e) => {
                oc_device::install::log_file::append(&[format!("speaker HID cannot be opened (retrying): {e}")]);
                self.ui.toast.show(ctx, crate::views::install::explain::inaccessible(&e));
            }
            _ => {}
        }
    }

    /// Shows the installer's write plan; closing the dialog without OK declines it.
    /// It opens once the bar shows the step that built the plan at 100 %.
    fn plan_prompt(&mut self, ctx: &Context) {
        let dialog_open = self.ui.dialog.is_some();
        let Some(inst) = self.device.installer() else { return };
        let progress = inst.progress();
        let (phase, plan) = (progress.phase, progress.plan.clone());
        match (phase, self.ui.hw.prompt) {
            (Phase::AwaitingConfirmation, Prompt::Idle) if !dialog_open => {
                let Some(plan) = plan else { return };
                if !crate::views::install::plan_ready(ctx, &progress) {
                    return;
                }
                let (title, body) = crate::views::install::plan_dialog(&progress, &plan);
                self.ui.hw.prompt = Prompt::Shown;
                self.dialog(title, body, tr("fw.install.ok"), true, Action::ConfirmInstall);
            }
            (Phase::AwaitingConfirmation, Prompt::Shown) if !dialog_open => {
                inst.answer(false);
                self.ui.hw.prompt = Prompt::Answered;
            }
            (Phase::AwaitingConfirmation, _) => {}
            _ => self.ui.hw.prompt = Prompt::Idle,
        }
    }

    pub(super) fn start_install(&mut self, ctx: &Context, kind: InstallKind) {
        let flow = match kind {
            InstallKind::First => Flow::First,
            InstallKind::Update => Flow::Update,
        };
        let Some(inst) = self.device.installer() else { return };
        let started = inst.open(flow) && inst.start();
        self.ui.toast.show(ctx, if started { tr("inst.warn") } else { tr("inst.err.title") });
    }

    pub(super) fn confirm_install(&mut self) {
        if let Some(inst) = self.device.installer() {
            inst.answer(true);
        }
        self.ui.hw.prompt = Prompt::Answered;
    }
}
