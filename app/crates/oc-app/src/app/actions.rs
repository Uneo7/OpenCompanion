use crate::model::{BandType, DeviceMode, FirmwarePart, StandbyDelay, Target};
use crate::widgets::eq_graph::Change;

use super::state::{Pane, Screen};

/// Installer flows the app offers on its own (stock firmware detected, DSP not ready).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    First,
    Update,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Show(Screen),
    ShowPane(Pane),
    SetTheme { light: bool },
    Minimize,
    AskClose,
    Close,

    SetVolume(u8),
    ToggleMute,
    ActivateProfile(usize),

    SetTarget(Target),
    SetLink(bool),
    ToggleCompare,
    ToggleBypass,
    ToggleLive,
    AskResetTarget,
    ResetTarget,
    /// Adds a band (at 1 kHz / 0 dB, or where double-clicked).
    AddBand(Option<(f64, f64)>),
    DeleteBand,
    SetBandType(usize, BandType),
    ZeroGain(usize),
    ToggleBand(usize),
    Edited(Change),
    AskRename(usize),
    Rename(usize),
    AskHue(usize),
    SetHue(usize),
    /// "Duplicate": copies profile `from` into slot `to` (never the locked one), which becomes active.
    Duplicate { from: usize, to: usize },
    /// The same into a used slot: asks before replacing it.
    AskDuplicate { from: usize, to: usize },
    AskFlatten(usize),
    Flatten(usize),
    /// EQ mode of the active profile's draft: Advanced (true: the global bands go to each output) or Simple.
    SetAdvanced(bool),
    AskSimple,

    ResetClips,
    SetLoudness(bool),
    SetDoubleTap(bool),
    SetMode(DeviceMode),
    SetStandby(StandbyDelay),
    SetHeadphoneEq(bool),
    AskInstall(FirmwarePart),
    Install(FirmwarePart),
    Backup,
    AskRestore,
    Restore,
    AskFactory,
    Factory,
    ConsoleSend,
    AskRestart(FirmwarePart),
    Restart(FirmwarePart),
    StartInstall(InstallKind),
    ConfirmInstall,
    /// View-only actions (dialogs opened from menus), applied by [`crate::views::actions::apply`].
    View(crate::views::actions::ViewAction),
}
