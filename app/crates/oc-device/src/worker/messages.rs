use oc_install::dsp::DspRuntime;

use crate::console::{BassSource, BassState, BootStatus, DeviceOptions, Route21, Meters, OpInfo, PodStatus, ProtectStatus, SlotEntry, SysStatus, Versions};
use crate::link::{DeviceMode, Presence, TasInfo};
use crate::params::Profile;

/// Slot of the built-in "Bose d'origine" profile: read-only, the worker refuses to save, rename or recolour it.
pub const LOCKED_SLOT: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartTarget {
    Dsp,
    Usb,
}

/// Consecutive [`Command::Write`] are coalesced (only the last one is sent).
// Profiles (~620 bytes) travel by value: a handful per second at most.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Only the differing bytes are written.
    Write(Profile),
    /// Loads slot 0..3 into the active profile; the new profile is read back and sent as [`Event::Profile`].
    Load(u8),
    Save { slot: u8, profile: Profile },
    /// Renames / recolours slot `slot`, which is not the active one, without changing what plays: loads `slot`
    /// (flat defaults if it is empty), patches its name / colour, saves it, loads `active` back (defaults if
    /// `None`) and writes `live` (the unsaved state the user hears) again. The sound changes for a moment.
    Patch { slot: u8, name: Option<String>, color: Option<u8>, active: Option<u8>, live: Profile },
    /// Fills the EMPTY slots among `slots` (slot, name, colour) with the Bose base and a flat EQ (`OPK`), then
    /// the name and colour, saved (`OPV`). A slot that holds a saved profile is never touched. The active slot
    /// is loaded back afterwards. Answered with [`Event::Provisioned`].
    Provision(Vec<(u8, String, u8)>),
    /// Copies the profile of slot `from` into slot `to` (never the locked slot) with `name` and `color`, saves
    /// it, and makes the copy the active profile (to edit it right away). An empty `from` copies the Bose base
    /// with a flat EQ.
    Duplicate { from: u8, to: u8, name: String, color: u8 },
    LoadBose,
    SetBassSource(BassSource),
    /// Sub level of the "app" source, dB × 10 (device setting, DSP protocol 8; saved by the DSP 1 s after the
    /// last change). Answered with [`Event::Bass`].
    SetSubLevel(i16),
    SetRoute21(bool),
    /// Answered with [`Event::Route21`].
    SetOptions(DeviceOptions),
    /// The speaker re-enumerates when it changes.
    SetMode(DeviceMode),
    /// Reply: [`Event::Console`] with the same `id`.
    Console { id: u64, line: String },
    /// Restarts a chip. The DSP is answered with [`Event::Restarted`] once it answers again; the USB controller
    /// ends the session (the speaker leaves the bus) and comes back as a new [`Event::Presence`].
    Restart(RestartTarget),
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DspState {
    Unknown,
    Runtime(DspRuntime),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Presence(Presence),
    Dsp(DspState),
    Versions(Versions),
    TasVersion(String),
    Boot(BootStatus),
    Usb(TasInfo),
    /// Device audio mode (`None`: unknown, e.g. no stream with a USB firmware without the mode command).
    Mode(Option<DeviceMode>),
    Slots(Vec<SlotEntry>),
    Info(OpInfo),
    Profile(Box<Profile>),
    Bass(BassState),
    Route21(Route21),
    Meters(Meters),
    Pod(PodStatus),
    Protect(ProtectStatus),
    Sys(SysStatus),
    Console { id: u64, reply: String },
    /// Empty: nothing was missing.
    Provisioned(Vec<u8>),
    Restarted { target: RestartTarget, ok: bool },
    Failed { what: String, error: String },
}
