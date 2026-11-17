use std::path::PathBuf;

use hyper_core::CoreError;

#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error(transparent)]
    Core(#[from] CoreError),

    #[error("HTTP request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("cannot build the HTTP client: {0}")]
    HttpClient(#[source] reqwest::Error),

    #[error("invalid JSON from {origin}: {source}")]
    Json {
        origin: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("hash mismatch for {path}: expected sha1 {expected}, got {actual}")]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("size mismatch for {path}: expected {expected} bytes, got {actual}")]
    SizeMismatch {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },

    #[error("invalid sha1 {0:?} in metadata")]
    InvalidSha1(String),

    #[error("version {0:?} not found in the Mojang manifest")]
    UnknownVersion(String),

    #[error("version {version}: unsupported metadata ({reason})")]
    UnsupportedVersion { version: String, reason: String },

    #[error("library {0:?} has no downloadable artifact with a hash")]
    UnsupportedLibrary(String),

    #[error("no Java runtime {component:?} published by Mojang for platform {platform:?}")]
    JavaRuntimeUnavailable { component: String, platform: String },

    #[error("Java runtime installed but {0} is missing")]
    JavaRuntimeIncomplete(PathBuf),

    #[error("this platform is not supported by Mojang's Java runtimes")]
    UnsupportedPlatform,

    #[error("cannot read archive {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },

    #[error("{loader} installer failed: {message}")]
    LoaderInstaller { loader: String, message: String },

    #[error("background task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub type Result<T, E = InstallerError> = std::result::Result<T, E>;
