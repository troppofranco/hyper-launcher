use hyper_core::CoreError;
use hyper_installer::InstallerError;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error(transparent)]
    Core(#[from] CoreError),

    /// In `Box` perché gli errori dell'installer sono grandi.
    #[error(transparent)]
    Installer(Box<InstallerError>),

    /// In `Box` come quelli dell'installer.
    #[error(transparent)]
    Content(Box<hyper_content::ContentError>),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("invalid data: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid server configuration: {0}")]
    Toml(String),

    #[error("invalid server name {0:?}: use letters, numbers, spaces, - and _ (max 32)")]
    InvalidName(String),

    #[error("{0}")]
    Invalid(String),

    #[error("server {0:?} not found")]
    NotFound(String),

    #[error("a server named {0:?} already exists")]
    AlreadyExists(String),

    #[error("server {0:?} is not running")]
    NotRunning(String),

    #[error("server {0:?} is already running")]
    AlreadyRunning(String),

    #[error("the Minecraft EULA must be accepted to create a server")]
    EulaRequired,

    #[error("{0} is not available for this version")]
    Unsupported(String),

    #[error("download URL not allowed: {0}")]
    UntrustedUrl(String),

    #[error("{file}: checksum mismatch")]
    HashMismatch { file: String },

    #[error("file too large to edit ({0} bytes, max 2 MB)")]
    FileTooLarge(u64),

    #[error("not a text file")]
    NotText,

    #[error("{0}")]
    Playit(String),

    #[error("the server host is not responding")]
    HostUnavailable,

    #[error("server host: {0}")]
    Host(String),
}

impl From<InstallerError> for ServerError {
    fn from(e: InstallerError) -> Self {
        Self::Installer(Box::new(e))
    }
}

pub type Result<T, E = ServerError> = std::result::Result<T, E>;
