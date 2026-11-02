//! Nothing in here depends on egui. The UI only sees the speaker through the [`Device`] trait: [`HwDevice`]
//! (the real speaker over USB, `oc-device`) or [`SimDevice`] (a simulated speaker, `--sim`).

pub mod biquad;
pub mod device;
pub mod eq;
pub mod format;
pub mod hw;
pub mod profile;
pub mod reference;
pub mod settings;
pub mod sim;

pub use device::{Connection, Device, DeviceMode, FirmwarePart, LinkState, StreamFormat};
pub use eq::{Band, BandType, Base, ProfileBands, Target};
pub use profile::{Profile, HUE_CHOICES};
pub use hw::HwDevice;
pub use settings::{DeviceSettings, StandbyDelay, SubSource};
pub use sim::SimDevice;
