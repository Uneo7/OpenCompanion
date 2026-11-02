//! Why no firmware release could be used. `Display` is the log / CLI form (English); the app shows its own
//! translated, actionable line for each case.

/// Start of the installer's error when it has no firmware to write (the reason follows).
pub const NO_FIRMWARE: &str = "no OpenCompanion firmware available";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseError {
    /// No network, DNS or TLS failure, timeout.
    NoConnection(String),
    RateLimited,
    /// Any other HTTP error status.
    Http { status: u16 },
    /// The repository has no published release at all.
    NoRelease { repo: String },
    /// Releases exist, none carries a firmware (zip + manifest).
    NoFirmware { repo: String },
    BadManifest(String),
}

impl std::fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoConnection(e) => write!(f, "no connection to GitHub ({e})"),
            Self::RateLimited => write!(f, "GitHub rate limit reached"),
            Self::Http { status } => write!(f, "GitHub answered HTTP {status}"),
            Self::NoRelease { repo } => write!(f, "no release published on GitHub ({repo})"),
            Self::NoFirmware { repo } => write!(f, "no firmware found in the GitHub releases ({repo})"),
            Self::BadManifest(e) => write!(f, "invalid firmware manifest: {e}"),
        }
    }
}
