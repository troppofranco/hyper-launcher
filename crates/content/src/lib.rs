//! Catalogo di contenuti da Modrinth: ricerca, installazione con dipendenze
//! in un'istanza, riconoscimento e aggiornamento dei file installati
//! (tramite il loro hash) e modpack `.mrpack`.
//!
//! Ogni file scaricato passa dal [`hyper_installer::Downloader`], che lo
//! verifica con SHA1 prima di metterlo al suo posto; ogni percorso che
//! arriva da fuori (nomi di file, percorsi nei modpack) è validato contro
//! il path traversal.

mod error;
pub mod install;
pub mod library;
pub mod modrinth;
pub mod mrpack;

use hyper_installer::LoaderKind;
use hyper_instances::ContentDir;
use serde::{Deserialize, Serialize};

pub use error::{ContentError, Result};
pub use install::{InstallPlan, PlannedFile};
pub use library::InstalledContent;
pub use modrinth::{Modrinth, Project, SearchHit, SearchQuery, SearchResults, SearchSort, Version};

/// Tipo di contenuto (il `project_type` di Modrinth).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Mod,
    ResourcePack,
    Shader,
    DataPack,
    Modpack,
}

impl ContentKind {
    pub fn project_type(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::ResourcePack => "resourcepack",
            Self::Shader => "shader",
            Self::DataPack => "datapack",
            Self::Modpack => "modpack",
        }
    }

    /// Cartella dell'istanza in cui finiscono i file di questo tipo (i data
    /// pack vanno dentro un mondo, i modpack diventano un'istanza).
    pub fn content_dir(self) -> Option<ContentDir> {
        match self {
            Self::Mod => Some(ContentDir::Mods),
            Self::ResourcePack => Some(ContentDir::ResourcePacks),
            Self::Shader => Some(ContentDir::ShaderPacks),
            Self::DataPack | Self::Modpack => None,
        }
    }

    /// Loader Modrinth delle versioni installabili in un'istanza con
    /// `loader` (vuoto = nessun filtro).
    pub fn loaders(self, loader: Option<LoaderKind>) -> Result<&'static [&'static str]> {
        use LoaderKind::{Fabric, Forge, NeoForge, Quilt};
        match (self, loader) {
            (Self::Mod, Some(Fabric)) => Ok(&["fabric"]),
            // Quilt carica anche le mod Fabric.
            (Self::Mod, Some(Quilt)) => Ok(&["quilt", "fabric"]),
            (Self::Mod, Some(NeoForge)) => Ok(&["neoforge"]),
            (Self::Mod, Some(Forge)) => Ok(&["forge"]),
            (Self::Mod, None) => Err(ContentError::NeedsLoader),
            // Un modpack porta con sé il suo loader: vanno bene tutti quelli
            // che sappiamo installare.
            (Self::Modpack, _) => Ok(&["fabric", "quilt", "neoforge", "forge"]),
            // Gli shader si caricano con Iris (Fabric, Quilt, NeoForge), che
            // legge anche il formato OptiFine.
            (Self::Shader, Some(Fabric | Quilt | NeoForge)) => Ok(&["iris", "optifine"]),
            (Self::Shader, _) => Err(ContentError::ShadersNeedIris),
            (Self::ResourcePack, _) => Ok(&["minecraft"]),
            (Self::DataPack, _) => Ok(&["datapack"]),
        }
    }
}

impl From<ContentDir> for ContentKind {
    fn from(dir: ContentDir) -> Self {
        match dir {
            ContentDir::Mods => Self::Mod,
            ContentDir::ResourcePacks => Self::ResourcePack,
            ContentDir::ShaderPacks => Self::Shader,
        }
    }
}

impl std::str::FromStr for ContentKind {
    type Err = ContentError;

    fn from_str(s: &str) -> Result<Self> {
        serde_json::from_value(serde_json::Value::String(s.to_owned()))
            .map_err(|_| ContentError::InvalidKind(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_and_map_loaders() {
        for kind in [
            ContentKind::Mod,
            ContentKind::ResourcePack,
            ContentKind::Shader,
            ContentKind::DataPack,
            ContentKind::Modpack,
        ] {
            assert_eq!(kind.project_type().parse::<ContentKind>().unwrap(), kind);
        }
        assert!("plugin".parse::<ContentKind>().is_err());
        assert!(matches!(
            ContentKind::Mod.loaders(None),
            Err(ContentError::NeedsLoader)
        ));
        assert!(matches!(
            ContentKind::Shader.loaders(None),
            Err(ContentError::ShadersNeedIris)
        ));
        assert_eq!(
            ContentKind::ResourcePack.loaders(None).unwrap(),
            &["minecraft"]
        );
    }
}
