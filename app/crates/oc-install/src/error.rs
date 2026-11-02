#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Link(#[from] oc_protocol::Error),
    #[error(transparent)]
    Load(#[from] oc_ramload::Error),
    #[error(transparent)]
    Ais(#[from] oc_ais::Error),
    #[error(transparent)]
    TasImage(#[from] oc_tas_image::Error),
    /// A safety check failed: nothing was written by the step that refused.
    #[error("refused: {0}")]
    Refused(String),
    #[error("device: {0}")]
    Device(String),
    #[error("verification failed: {0}")]
    Verify(String),
    #[error("backup files: {0}")]
    File(String),
    #[error("cancelled by the user (nothing was written)")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn refused<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Refused(msg.into()))
}
