//! The verified backup of the speaker's Bose firmware (`InstallController::stock_backup`): taken automatically
//! when a speaker still running the stock firmware connects, shown in Settings › Firmware, and required by the
//! first install and the restore (both are refused without it).

use eframe::egui::Context;
use oc_device::install::{BackupState, Flow};

use super::super::state;
use crate::i18n::{t_with, tr};
use crate::model::Connection;
use crate::views::Cx;

pub fn backup_state(cx: &mut Cx) -> BackupState {
    let sim_date = cx.device.last_backup();
    match cx.device.installer() {
        Some(i) => i.stock_backup(),
        None => BackupState::Saved { date: sim_date, dir: Default::default(), created: false },
    }
}

pub fn watch(ctx: &Context, cx: &mut Cx) {
    if cx.device.connection() != Connection::Stock {
        state::with(ctx, |s| s.backup_tried = false);
        return;
    }
    if state::with(ctx, |s| s.backup_tried) {
        return;
    }
    let Some(inst) = cx.device.installer() else { return };
    if matches!(inst.stock_backup(), BackupState::Missing | BackupState::Failed(_)) {
        inst.ensure_stock_backup();
    }
    state::with(ctx, |s| s.backup_tried = true);
}

pub fn request(cx: &mut Cx) -> bool {
    match cx.device.installer() {
        Some(inst) => {
            inst.ensure_stock_backup();
            true
        }
        None => false,
    }
}

fn date(d: &str) -> String {
    let p: Vec<&str> = d.split('-').collect();
    match p.as_slice() {
        [y, m, dd] if y.len() == 4 => format!("{dd}/{m}/{y}"),
        _ => d.to_owned(),
    }
}

pub fn status_text(s: &BackupState) -> String {
    match s {
        BackupState::Saved { date: d, .. } => t_with("fw.backup.ok", &[("date", &date(d))]),
        BackupState::Running => tr("fw.backup.running").to_owned(),
        BackupState::Missing => tr("fw.backup.missing").to_owned(),
        BackupState::Failed(e) => super::explain::backup_failure(e, None),
    }
}

/// Only the restore needs a backup beforehand: the first install makes (and verifies) it as its first step.
pub fn blocks(flow: &Flow, s: &BackupState) -> bool {
    matches!(flow, Flow::Restore) && !matches!(s, BackupState::Saved { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_blocking() {
        assert_eq!(date("2026-10-12"), "12/10/2026");
        assert_eq!(date("x"), "x");
        assert!(blocks(&Flow::Restore, &BackupState::Missing));
        assert!(!blocks(&Flow::Update, &BackupState::Missing));
        assert!(!blocks(&Flow::First, &BackupState::Missing), "the first install makes the backup itself");
        assert!(!blocks(&Flow::First, &BackupState::Saved { date: "d".into(), dir: Default::default(), created: true }));
    }
}
