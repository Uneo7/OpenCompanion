pub mod dsp;
pub mod link;
pub mod oc_hid;
pub mod tas_eeprom;
pub mod transport;

pub use link::Link;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HID : {0}")]
    Hid(#[from] hidapi::HidError),
    #[error("speaker not found (VID 05A7, PID 1020)")]
    NotFound,
    #[error("no response to command '{0}'")]
    Timeout(char),
    #[error("payload too long ({0} bytes, 60 max)")]
    TooLong(usize),
    #[error("unexpected response: {0}")]
    Unexpected(String),
    #[error("TAS relay reported \"{0}\"")]
    Relay(String),
    #[error("command rejected by the DSP (NAK)")]
    Nak,
    #[error("OpenCompanion TAS firmware returned status 0x{0:02x}")]
    Status(u8),
}

pub type Result<T> = std::result::Result<T, Error>;
