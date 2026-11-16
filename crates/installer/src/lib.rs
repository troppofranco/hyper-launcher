//! Installer di versioni vanilla: version manifest Mojang, client jar,
//! librerie, asset, configurazione di logging e runtime Java, tutti scaricati
//! in parallelo nella cache condivisa e verificati con SHA1 prima dell'uso.

pub mod assets;
pub mod download;
mod error;
pub mod hash;
mod install;
pub mod java;
pub mod manifest;
pub mod natives;
pub mod version;

pub use download::{DownloadStats, DownloadTask, Downloader, Progress};
pub use error::{InstallerError, Result};
pub use hash::Sha1Hex;
pub use install::{InstalledVersion, Installer, LoggingConfig, NativeJar, Stage, StageProgressFn};
pub use manifest::{ManifestVersion, VersionKind, VersionManifest};
pub use version::{RuleContext, VersionJson};
