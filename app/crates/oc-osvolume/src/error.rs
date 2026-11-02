use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Error reported by the OS audio API (HRESULT, OSStatus or ALSA errno).
    #[error("{api}: {message}")]
    Os { api: &'static str, message: String },
    #[error("device has no {0} control")]
    NoControl(&'static str),
    #[error("OS volume control is not supported on this platform")]
    Unsupported,
}

impl Error {
    #[allow(dead_code)] // unused on platforms without a backend
    pub(crate) fn os(api: &'static str, message: impl ToString) -> Self {
        Error::Os {
            api,
            message: message.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
