use std::path::PathBuf;

use hyper_core::CoreError;
use hyper_installer::InstallerError;
use hyper_instances::InstanceError;

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error(transparent)]
    Core(#[from] CoreError),

    /// In `Box` perché gli errori dell'installer e delle istanze sono grandi.
    #[error(transparent)]
    Installer(Box<InstallerError>),

    #[error(transparent)]
    Instance(Box<InstanceError>),

    #[error("Modrinth request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("invalid response from {url}: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid Modrinth id {0:?}")]
    InvalidId(String),

    #[error("unknown content type {0:?}")]
    InvalidKind(String),

    #[error("{project} has no version compatible with Minecraft {game}")]
    NoCompatibleVersion { project: String, game: String },

    #[error("mods need an instance with a mod loader (Fabric, Quilt, NeoForge or Forge)")]
    NeedsLoader,

    #[error("shaders need an instance with Fabric, Quilt or NeoForge (they run through Iris)")]
    ShadersNeedIris,

    #[error("choose the world to install the data pack into")]
    WorldRequired,

    #[error("world {0:?} not found in this instance")]
    WorldNotFound(String),

    #[error("modpacks are installed as a new instance")]
    ModpackIsNotContent,

    #[error("this modpack needs {0}, which is not supported yet")]
    UnsupportedLoader(String),

    #[error("download URL not allowed: {0}")]
    UntrustedUrl(String),

    #[error("invalid modpack: {0}")]
    InvalidPack(String),

    #[error("background task failed: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("zip error in {path}: {source}")]
    Zip {
        path: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
}

impl From<InstallerError> for ContentError {
    fn from(e: InstallerError) -> Self {
        Self::Installer(Box::new(e))
    }
}

impl From<InstanceError> for ContentError {
    fn from(e: InstanceError) -> Self {
        Self::Instance(Box::new(e))
    }
}

pub type Result<T, E = ContentError> = std::result::Result<T, E>;
