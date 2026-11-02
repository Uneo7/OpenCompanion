use crate::model::{Connection, Device, ProfileBands, Target};
use crate::widgets::{dialog::Dialog, menu::Menu, toast::Toast};

use super::actions::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Equalizer,
    Settings,
}

impl Screen {
    pub const ALL: [Screen; 3] = [Screen::Home, Screen::Equalizer, Screen::Settings];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Status,
    Sound,
    Outputs,
    Device,
    Firmware,
    /// App preferences (appearance, language): nothing is sent to the speaker.
    Prefs,
    Advanced,
    Legal,
}

impl Pane {
    pub const ALL: [Pane; 8] =
        [Pane::Status, Pane::Sound, Pane::Outputs, Pane::Device, Pane::Firmware, Pane::Prefs, Pane::Advanced, Pane::Legal];
}

pub struct EqState {
    pub target: Target,
    pub driver: Target,
    pub selected: usize,
    pub link: bool,
    pub compare: bool,
    pub bypass: bool,
    /// Changes are played right away (otherwise when saved).
    pub live: bool,
    /// Changes in `draft` not saved to the speaker yet (auto-saved, see `App::auto_save`).
    pub dirty: bool,
    /// Counts the edits of `draft`: the auto-save waits for it to stay still.
    pub edits: u64,
    /// When the last edit happened (s), for the debounce.
    pub edited_at: f64,
    /// The edit count whose save failed: no retry until the next change.
    pub failed_at_edit: Option<u64>,
    pub draft: ProfileBands,
    /// `draft` after the last edit, within the boost limit: where a gesture over the limit is moved back to.
    pub last_ok: ProfileBands,
    /// The boost limit bit since the hint was last shown.
    pub limited: bool,
}

impl EqState {
    pub fn new(draft: ProfileBands) -> Self {
        Self {
            target: Target::Global,
            driver: Target::InnerLeft,
            selected: 3,
            link: true,
            compare: false,
            bypass: false,
            live: true,
            dirty: false,
            edits: 0,
            edited_at: 0.0,
            failed_at_edit: None,
            last_ok: draft.clone(),
            draft,
            limited: false,
        }
    }

    /// Edits `bands` (a profile from the speaker or a file), scaled within the boost limit; true when it was
    /// (the scaled bands are then saved like an edit).
    pub fn load(&mut self, mut bands: ProfileBands) -> bool {
        let scaled = crate::model::boost::fit(&mut bands);
        self.last_ok = bands.clone();
        self.draft = bands;
        self.selected = 0;
        if scaled {
            self.dirty = true;
            self.edits += 1;
        }
        scaled
    }
}

pub struct SettingsState {
    pub pane: Pane,
    pub console: String,
    pub console_input: String,
    pub updating: [bool; 2],
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            pane: Pane::Status,
            console: "oc-dsp 0.9.3 · tapez « help » ou « peek 0x0400 »\n".into(),
            console_input: String::new(),
            updating: [false; 2],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Prompt {
    #[default]
    Idle,
    Shown,
    Answered,
}

#[derive(Debug, Default)]
pub struct HwUi {
    pub connection: Option<Connection>,
    pub prompt: Prompt,
    /// An update check is due (new connection of a ready speaker).
    pub check_pending: bool,
    /// Settings › Firmware was shown last frame (to refresh the check when it opens).
    pub on_firmware: bool,
    /// The window had the focus last frame (launch and a regained focus refresh the check).
    pub focused: bool,
}

pub struct Closing {
    pub dialog: Dialog<Action>,
    pub since: f64,
}

pub struct UiState {
    pub screen: Screen,
    pub light: bool,
    pub eq: EqState,
    pub settings: SettingsState,
    pub menu: Option<Menu<Action>>,
    pub dialog: Option<Dialog<Action>>,
    pub closing: Option<Closing>,
    pub toast: Toast,
    /// Actions raised by views this frame, applied after drawing.
    pub pending: Vec<Action>,
    /// Counter for menu / dialog identities (entrance animations).
    pub serial: u64,
    pub demo: Option<String>,
    pub hw: HwUi,
    /// The user confirmed "Fermer": the next close request of the window goes through.
    pub close_confirmed: bool,
    /// A native file dialog to open at the end of the frame (it needs the window as its parent).
    pub pick: Option<crate::app::files::Pick>,
}

impl UiState {
    pub fn new(device: &dyn Device) -> Self {
        let draft = device.profiles()[device.active_profile()].bands.clone();
        Self {
            screen: Screen::Home,
            light: false,
            eq: EqState::new(draft),
            settings: SettingsState::default(),
            menu: None,
            dialog: None,
            closing: None,
            toast: Toast::default(),
            pending: Vec::new(),
            serial: 0,
            demo: None,
            hw: HwUi::default(),
            close_confirmed: false,
            pick: None,
        }
    }

    pub fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    pub fn act(&mut self, a: Action) {
        self.pending.push(a);
    }
}
