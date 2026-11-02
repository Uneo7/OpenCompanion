#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Link(#[from] oc_protocol::Error),
    /// `15 <code>` reply.
    #[error("DSP refused {cmd}: {}", nak_text(*.code))]
    Nak { cmd: &'static str, code: u8 },
    #[error("unexpected DSP reply to {cmd}: {reply}")]
    Reply { cmd: &'static str, reply: String },
    #[error(transparent)]
    Layout(#[from] crate::params::LayoutError),
    #[error("{0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// `PR_*` result codes of the profile commands (`firmware/dsp/include/profiles.h`).
pub fn nak_text(code: u8) -> &'static str {
    match code {
        1 => "busy (EEPROM job running)",
        2 => "EEPROM I/O error",
        3 => "verify error",
        4 => "CRC error",
        5 => "protected (EEPROM area not ours)",
        6 => "empty slot",
        7 => "bad argument",
        _ => "refused",
    }
}

impl Error {
    /// The link itself failed (device gone, HID error, timeout): the worker reconnects.
    pub fn is_transport(&self) -> bool {
        matches!(self, Error::Link(oc_protocol::Error::Hid(_) | oc_protocol::Error::Timeout(_) | oc_protocol::Error::NotFound))
    }
}
