use eframe::egui::{Context, ViewportCommand};
use oc_device::install::Phase;

use crate::i18n::{t_with, tr};
use crate::model::{eq::MAX_BANDS, Band, FirmwarePart, ProfileBands, Target, HUE_CHOICES};
use crate::theme;
use crate::widgets::{dialog::Body, dialog::Dialog, eq_graph::Change};

use super::{actions::Action, state::Closing, App};

impl App {
    fn active(&self) -> usize {
        self.device.active_profile()
    }

    fn profile_name(&self, i: usize) -> String {
        crate::views::profile_name(&self.device.profiles()[i], i)
    }

    pub(super) fn dialog(&mut self, title: impl Into<String>, body: Body, ok: &str, danger: bool, action: Action) {
        let serial = self.ui.next_serial();
        self.ui.menu = None;
        self.ui.dialog = Some(Dialog { title: title.into(), body, ok: ok.into(), danger, action, serial });
    }

    pub(super) fn close_dialog(&mut self, ctx: &Context) {
        if let Some(d) = self.ui.dialog.take() {
            self.ui.closing = Some(Closing { dialog: d, since: ctx.input(|i| i.time) });
        }
    }

    fn edited(&mut self, kind: Change) {
        if kind == Change::Select {
            return;
        }
        let eq = &mut self.ui.eq;
        // The combined boost of an output never goes over the limit: the gesture stops there.
        if crate::model::boost::limit_profile_edit(&eq.last_ok, &mut eq.draft) {
            eq.limited = true;
        }
        if eq.link {
            eq.draft.mirror_from(eq.target);
        }
        eq.last_ok = eq.draft.clone();
        eq.dirty = true;
        eq.edits += 1;
        if eq.live {
            self.device.preview(Some(&self.ui.eq.draft));
        }
    }

    /// Saves the draft to the active profile's slot (quietly: changes are always saved). A failure shows one
    /// toast and waits for the next change to try again. The locked profile is never saved.
    pub(super) fn save(&mut self, ctx: &Context) {
        if !self.ui.eq.dirty {
            return;
        }
        let i = self.active();
        if self.device.profiles().get(i).is_some_and(|p| p.locked) {
            self.ui.eq.dirty = false;
            return;
        }
        if self.device.store_profile(i, &self.ui.eq.draft) {
            self.ui.eq.dirty = false;
            self.ui.eq.failed_at_edit = None;
        } else if self.ui.eq.failed_at_edit != Some(self.ui.eq.edits) {
            self.ui.eq.failed_at_edit = Some(self.ui.eq.edits);
            self.ui.toast.show(ctx, tr("eq.autosave.failed"));
        }
    }

    /// Auto-save, debounced: 1.5 s after the last edit, never while a button is held (a drag in progress), and
    /// not again after a failure until the next change. Called once per frame.
    pub(super) fn auto_save(&mut self, ctx: &Context) {
        const DELAY_S: f64 = 1.5;
        // The boost limit bit: one short hint (not again while the same gesture goes on).
        if std::mem::take(&mut self.ui.eq.limited) {
            let now = ctx.input(|i| i.time);
            let key = eframe::egui::Id::new("boost-hint");
            if ctx.data(|d| d.get_temp::<f64>(key)).is_none_or(|t| now - t > 4.0) {
                self.ui.toast.show(ctx, tr("eq.boost.limited"));
            }
            ctx.data_mut(|d| d.insert_temp(key, now));
        }
        let eq = &mut self.ui.eq;
        if !eq.dirty || eq.failed_at_edit == Some(eq.edits) {
            return;
        }
        let now = ctx.input(|i| i.time);
        let key = eframe::egui::Id::new("autosave-edits");
        if ctx.data(|d| d.get_temp::<u64>(key)) != Some(eq.edits) {
            ctx.data_mut(|d| d.insert_temp(key, eq.edits));
            eq.edited_at = now;
        }
        let holding = ctx.input(|i| i.pointer.any_down());
        if !holding && now - eq.edited_at >= DELAY_S {
            self.save(ctx);
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// The profile you hear is the profile you edit: switching saves pending changes first.
    fn activate(&mut self, ctx: &Context, i: usize) {
        if i == self.active() {
            return;
        }
        if self.ui.eq.dirty {
            self.save(ctx);
        }
        self.device.set_active_profile(i);
        let bands = self.device.profiles()[i].bands.clone();
        if self.ui.eq.load(bands) {
            self.ui.toast.show(ctx, tr("eq.boost.scaled"));
        }
    }

    fn duplicate(&mut self, ctx: &Context, from: usize, to: usize) {
        if self.ui.eq.dirty {
            self.save(ctx);
        }
        let profiles = self.device.profiles();
        let name = duplicate_name(&profiles[from], from);
        let hue = next_free_hue(profiles, to);
        if self.device.duplicate(from, to, &name, hue) {
            self.ui.eq.dirty = false;
            self.ui.eq.load(self.device.profiles()[to].bands.clone());
            self.ui.toast.show(ctx, t_with("profile.duplicate.done", &[("name", &name), ("n", &(to + 1))]));
        }
    }

    fn replace_draft(&mut self, ctx: &Context, bands: ProfileBands, toast: String) {
        self.ui.eq.draft = bands;
        self.ui.eq.selected = 0;
        self.edited(Change::Struct);
        self.ui.toast.show(ctx, toast);
    }

    fn current_len(&self) -> usize {
        self.ui.eq.draft.get(self.ui.eq.target).len()
    }

    /// Applies `action`. `body` is the dialog's content when the action comes from its OK button.
    /// Returns false to keep that dialog open.
    pub(super) fn apply(&mut self, ctx: &Context, action: Action, body: Option<&Body>) -> bool {
        // "Bose® d'origine" is read-only: every edit of it is ignored (the device refuses to write it anyway).
        if self.edits_locked(&action) {
            return true;
        }
        let toast = |app: &mut App, m: String| app.ui.toast.show(ctx, m);
        match action {
            Action::Show(s) => {
                self.ui.screen = s;
                self.ui.menu = None;
            }
            Action::ShowPane(p) => self.ui.settings.pane = p,
            Action::SetTheme { light } => self.ui.light = light,
            Action::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
            Action::AskClose => self.ask_close(),
            Action::Close => {
                if self.installer_writing() {
                    // An interrupted write would leave a chip without firmware: the window stays open.
                    self.ui.toast.show(ctx, tr("inst.warn.unplug"));
                } else {
                    self.save(ctx);
                    self.ui.close_confirmed = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }

            Action::SetVolume(v) => self.device.set_volume(v.min(100)),
            Action::ToggleMute => {
                let m = self.device.muted();
                self.device.set_muted(!m);
            }
            Action::ActivateProfile(i) => self.activate(ctx, i),

            Action::SetTarget(t) => {
                self.ui.eq.target = t;
                if t.is_advanced() {
                    self.ui.eq.driver = t;
                }
                self.ui.eq.selected = 0;
            }
            Action::SetLink(on) => {
                self.ui.eq.link = on;
                toast(self, tr(if on { "eq.link.on" } else { "eq.link.off" }).into());
            }
            Action::ToggleCompare => {
                self.ui.eq.compare = !self.ui.eq.compare;
                self.device.set_compare(self.ui.eq.compare);
            }
            Action::ToggleBypass => {
                self.ui.eq.bypass = !self.ui.eq.bypass;
                self.device.set_bypass(self.ui.eq.bypass);
            }
            Action::ToggleLive => {
                self.ui.eq.live = !self.ui.eq.live;
                let m = tr(if self.ui.eq.live { "eq.live.on" } else { "eq.live.off" });
                toast(self, m.into());
            }
            Action::SetAdvanced(true) => {
                if !self.ui.eq.draft.advanced {
                    self.ui.eq.draft.to_advanced();
                    self.ui.eq.target = self.ui.eq.driver;
                    self.ui.eq.selected = 0;
                    self.edited(Change::Struct);
                }
            }
            Action::AskSimple => {
                let body = Body::Text(tr("eq.simple.body").into());
                self.dialog(tr("eq.simple.title"), body, tr("eq.simple.ok"), true, Action::SetAdvanced(false));
            }
            Action::SetAdvanced(false) => {
                if self.ui.eq.draft.advanced {
                    self.ui.eq.draft.to_simple();
                    self.ui.eq.target = Target::Global;
                    self.ui.eq.selected = 0;
                    self.edited(Change::Struct);
                }
            }
            Action::AskResetTarget => {
                let label = tr(crate::widgets::fmt::target_key(self.ui.eq.target));
                self.dialog(
                    t_with("eq.reset.title", &[("target", &label)]),
                    Body::Text(tr("eq.reset.body").into()),
                    tr("eq.reset.ok"),
                    true,
                    Action::ResetTarget,
                );
            }
            Action::ResetTarget => {
                let t = self.ui.eq.target;
                self.ui.eq.draft.get_mut(t).clear();
                self.ui.eq.selected = 0;
                self.edited(Change::Struct);
            }
            Action::AddBand(at) => {
                if self.ui.eq.compare {
                    return true;
                }
                if self.current_len() >= MAX_BANDS {
                    toast(self, t_with("eq.max", &[("n", &MAX_BANDS)]));
                    return true;
                }
                let (f, g) = at.unwrap_or((1000.0, 0.0));
                let t = self.ui.eq.target;
                let list = self.ui.eq.draft.get_mut(t);
                list.push(Band::created_at(f, g));
                self.ui.eq.selected = list.len() - 1;
                self.edited(Change::Struct);
            }
            Action::DeleteBand => {
                let (t, sel) = (self.ui.eq.target, self.ui.eq.selected);
                let list = self.ui.eq.draft.get_mut(t);
                if sel < list.len() {
                    list.remove(sel);
                    self.ui.eq.selected = sel.min(list.len().saturating_sub(1));
                    self.edited(Change::Struct);
                }
            }
            Action::SetBandType(i, kind) => self.edit_band(i, Change::Struct, |b| b.kind = kind),
            Action::ZeroGain(i) => self.edit_band(i, Change::Value, |b| b.gain = 0.0),
            Action::ToggleBand(i) => self.edit_band(i, Change::Struct, |b| b.on = !b.on),
            Action::Edited(kind) => self.edited(kind),
            Action::AskRename(i) => {
                let value = self.profile_name(i);
                self.dialog(tr("profile.rename.title"), Body::Field { value, max_len: 24 }, tr("profile.rename.ok"), false, Action::Rename(i));
            }
            Action::Rename(i) => {
                if let Some(Body::Field { value, .. }) = body {
                    if value.trim().is_empty() {
                        return false;
                    }
                    self.device.rename_profile(i, value);
                }
            }
            Action::AskHue(i) => {
                let p = &self.device.profiles()[i];
                let hues = HUE_CHOICES.iter().map(|h| (theme::rgb(h.a), theme::rgb(h.b))).collect();
                let title = t_with("profile.color.title", &[("name", &self.profile_name(i))]);
                let selected = p.hue;
                // Each colour belongs to one profile at most: the others' colours are not offered.
                let taken = (0..HUE_CHOICES.len())
                    .map(|c| (0..self.device.profiles().len()).find(|j| *j != i && self.device.profiles()[*j].hue == c).map(|j| self.profile_name(j)))
                    .collect();
                self.dialog(title, Body::Hues { hues, selected, taken }, tr("profile.color.ok"), false, Action::SetHue(i));
            }
            Action::SetHue(i) => {
                if let Some(Body::Hues { selected, .. }) = body {
                    self.device.set_profile_hue(i, *selected);
                }
            }
            Action::AskDuplicate { from, to } => {
                let (src, dst) = (self.profile_name(from), self.profile_name(to));
                let body = Body::Text(t_with("profile.duplicate.body", &[("name", &dst), ("from", &src)]));
                let title = t_with("profile.duplicate.title", &[("name", &dst)]);
                self.dialog(title, body, tr("profile.duplicate.ok"), true, Action::Duplicate { from, to });
            }
            Action::Duplicate { from, to } => self.duplicate(ctx, from, to),
            Action::AskFlatten(i) => {
                let name = self.profile_name(i);
                let title = t_with("profile.flat.title", &[("name", &name)]);
                self.dialog(title, Body::Text(tr("profile.flat.body").into()), tr("profile.flat"), true, Action::Flatten(i));
            }
            Action::Flatten(i) => {
                let name = self.profile_name(i);
                self.replace_draft(ctx, ProfileBands::bose(), t_with("profile.flat.done", &[("name", &name)]));
            }

            Action::ResetClips => {
                self.device.reset_clips();
                toast(self, tr("settings.clips.reset").into());
            }
            Action::SetLoudness(on) => {
                self.update_settings(|s| s.loudness = on);
                toast(self, tr(if on { "settings.loud.on" } else { "settings.loud.off" }).into());
            }
            Action::SetDoubleTap(on) => {
                self.update_settings(|s| s.options.double_tap = on);
                toast(self, tr(if on { "settings.cycle.on" } else { "settings.cycle.off" }).into());
            }
            Action::SetMode(m) => self.device.set_mode(m),
            Action::SetStandby(s) => self.update_settings(|x| x.options.standby = s),
            Action::SetHeadphoneEq(on) => self.update_settings(|s| s.options.headphone_eq = on),
            Action::AskInstall(part) => {
                let file = match part {
                    FirmwarePart::Usb => "oc-usb.bin",
                    FirmwarePart::Dsp => "oc-dsp.bin",
                };
                self.dialog(
                    t_with("fw.file.title", &[("file", &file)]),
                    Body::Text(tr("fw.file.body").into()),
                    tr("fw.install.ok"),
                    false,
                    Action::Install(part),
                );
            }
            Action::Install(part) => self.device.begin_update(part),
            Action::Backup => {
                self.device.backup();
                toast(self, tr("fw.backup.done").into());
            }
            Action::AskRestore => {
                let date = self.device.last_backup();
                self.dialog(
                    tr("fw.restore.title"),
                    Body::Text(t_with("fw.restore.body", &[("date", &date)])),
                    tr("fw.restore.ok"),
                    true,
                    Action::Restore,
                );
            }
            Action::Restore => {
                self.device.restore_backup();
                toast(self, tr("fw.restore.started").into());
            }
            Action::AskFactory => self.dialog(
                tr("factory.title"),
                Body::Confirm { text: tr("factory.body").into(), placeholder: tr("factory.ph").into(), value: String::new() },
                tr("fw.restore.ok"),
                true,
                Action::Factory,
            ),
            Action::Factory => {
                if let Some(Body::Confirm { value, .. }) = body {
                    if value.trim().to_uppercase() != "BOSE" {
                        return false;
                    }
                }
                self.device.restore_factory();
                toast(self, tr("fw.restore.started").into());
            }
            Action::AskRestart(part) => {
                let title = tr(if part == FirmwarePart::Dsp { "settings.advanced.restart.dsp" } else { "settings.advanced.restart.usb" });
                let body = Body::Text(tr("settings.advanced.restart.body").into());
                self.dialog(title, body, tr("settings.advanced.restart.ok"), false, Action::Restart(part));
            }
            Action::Restart(part) => self.device.restart(part),
            Action::ConsoleSend => {
                let cmd = self.ui.settings.console_input.trim().to_owned();
                if !cmd.is_empty() {
                    let out = self.device.console(&cmd);
                    // The hardware answers later (`watch_hardware` appends the reply).
                    self.ui.settings.console +=
                        &if out.is_empty() { format!("> {cmd}\n") } else { format!("> {cmd}\n{out}\n") };
                    self.ui.settings.console_input.clear();
                }
            }
            Action::StartInstall(kind) => self.start_install(ctx, kind),
            Action::ConfirmInstall => self.confirm_install(),
            Action::View(v) => {
                let mut cx = crate::views::Cx { device: self.device.as_mut(), ui: &mut self.ui };
                return crate::views::actions::apply(ctx, &mut cx, v, body);
            }
        }
        true
    }

    pub(super) fn installer_writing(&mut self) -> bool {
        self.device.installer().is_some_and(|i| i.is_running() && i.with_progress(|p| p.writing()))
    }

    /// Window button or a close request from the OS: closes at once. While the installer writes, a dialog only
    /// explains why the window cannot close now.
    fn ask_close(&mut self) {
        if self.installer_writing() {
            let action = Action::View(crate::views::actions::ViewAction::Dismiss);
            self.dialog(tr("app.close.title"), Body::Text(tr("inst.warn.unplug").into()), tr("dialog.ok"), false, action);
            return;
        }
        self.ui.act(Action::Close);
    }

    fn edit_band(&mut self, i: usize, kind: Change, f: impl FnOnce(&mut Band)) {
        let t = self.ui.eq.target;
        if let Some(b) = self.ui.eq.draft.get_mut(t).get_mut(i) {
            f(b);
            self.ui.eq.selected = i;
            self.edited(kind);
        }
    }

    fn update_settings(&mut self, f: impl FnOnce(&mut crate::model::DeviceSettings)) {
        let mut s = self.device.settings().clone();
        f(&mut s);
        self.device.set_settings(s);
    }

    pub(super) fn watch_device(&mut self, ctx: &Context) {
        self.auto_save(ctx);
        for (k, part) in [FirmwarePart::Usb, FirmwarePart::Dsp].into_iter().enumerate() {
            let running = self.device.update_progress(part).is_some();
            // A flow that ended: success toast only when it finished; a declined plan says it was cancelled
            // (nothing written); a failure is reported by the hardware notice.
            if self.ui.settings.updating[k] && !running {
                match self.device.installer().map(|i| i.phase()) {
                    None | Some(Phase::Done) => self.ui.toast.show(ctx, tr("fw.done")),
                    Some(_) => {}
                }
            }
            self.ui.settings.updating[k] = running;
        }
        // Last, so that a hardware notice (e.g. an install failure) wins over the generic toasts above.
        self.watch_hardware(ctx);
        let len = self.ui.eq.draft.get(self.ui.eq.target).len();
        if self.ui.eq.selected >= len && len > 0 {
            self.ui.eq.selected = len - 1;
        }
    }
}

impl App {
    fn edits_locked(&self, action: &Action) -> bool {
        let locked = |i: usize| self.device.profiles().get(i).is_some_and(|p| p.locked);
        match action {
            Action::AddBand(_)
            | Action::DeleteBand
            | Action::SetBandType(..)
            | Action::ZeroGain(_)
            | Action::ToggleBand(_)
            | Action::Edited(_)
            | Action::AskResetTarget
            | Action::ResetTarget
            | Action::SetAdvanced(_)
            | Action::AskSimple
            | Action::SetLoudness(_) => locked(self.device.active_profile()),
            Action::AskRename(i) | Action::Rename(i) | Action::AskHue(i) | Action::SetHue(i) | Action::AskFlatten(i) | Action::Flatten(i) => locked(*i),
            Action::Duplicate { to, .. } | Action::AskDuplicate { to, .. } => locked(*to),
            Action::View(crate::views::actions::ViewAction::AskImport(i)) | Action::View(crate::views::actions::ViewAction::Import(i, _)) => locked(*i),
            _ => false,
        }
    }
}

/// Name of a copy of profile `i`: "<name> (copie)", within the speaker's 16 bytes (the name part is shortened,
/// never the suffix). The locked "Bose® d'origine" gives "Bose (copie)".
pub(crate) fn duplicate_name(p: &crate::model::Profile, i: usize) -> String {
    let base = if p.locked { "Bose".to_owned() } else { crate::views::profile_name(p, i) };
    let fit = |b: &str| t_with("profile.duplicate.name", &[("name", &b)]);
    let mut b: String = base;
    while fit(&b).len() > oc_device::params::NAME_LEN && !b.is_empty() {
        b.pop();
        b = b.trim_end().to_owned();
    }
    fit(&b)
}

pub(crate) fn next_free_hue(profiles: &[crate::model::Profile], to: usize) -> usize {
    let used: Vec<usize> = profiles.iter().enumerate().filter(|(k, _)| *k != to).map(|(_, p)| p.hue % HUE_CHOICES.len()).collect();
    (0..HUE_CHOICES.len()).find(|h| !used.contains(h)).unwrap_or(to)
}
