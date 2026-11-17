//! Mod loader. Per ora Fabric: la sua API pubblica un profilo (version
//! JSON con `inheritsFrom`) da fondere sulla versione vanilla.

use hyper_core::safe_component;
use serde::{Deserialize, Serialize};

use crate::install::{parse_json, read_json};
use crate::version::VersionJson;
use crate::{Installer, InstallerError, Result};

pub const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Fabric,
}

impl LoaderKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
        }
    }
}

/// Loader scelto per un'istanza (salvato in `instance.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderSpec {
    pub kind: LoaderKind,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderEntry {
    loader: LoaderVersion,
}

impl Installer {
    /// Versioni del loader disponibili per una versione di gioco, dalla più
    /// recente (vuoto se il loader non supporta quella versione).
    pub async fn loader_versions(
        &self,
        kind: LoaderKind,
        game: &str,
    ) -> Result<Vec<LoaderVersion>> {
        let LoaderKind::Fabric = kind;
        let url = format!(
            "{FABRIC_META_URL}/versions/loader/{}",
            safe_component(game)?
        );
        let bytes = self.downloader.get_bytes(&url).await?;
        let entries: Vec<FabricLoaderEntry> = parse_json(&bytes, &url)?;
        Ok(entries.into_iter().map(|e| e.loader).collect())
    }

    /// Ultima versione stabile del loader per `game`.
    pub async fn latest_loader(&self, kind: LoaderKind, game: &str) -> Result<LoaderSpec> {
        let versions = self.loader_versions(kind, game).await?;
        let version = versions
            .iter()
            .find(|v| v.stable)
            .or(versions.first())
            .ok_or_else(|| InstallerError::UnsupportedVersion {
                version: game.to_owned(),
                reason: format!("{} does not support it", kind.display_name()),
            })?;
        Ok(LoaderSpec {
            kind,
            version: version.version.clone(),
        })
    }

    /// Profilo del loader per `game`, salvato in `versions/` come cache
    /// per l'uso offline. Come il version manifest, arriva in HTTPS senza
    /// hash noto; le librerie che elenca sono invece verificate una a una.
    pub(crate) async fn loader_profile(
        &self,
        spec: &LoaderSpec,
        game: &str,
    ) -> Result<VersionJson> {
        let LoaderKind::Fabric = spec.kind;
        let (game, loader) = (safe_component(game)?, safe_component(&spec.version)?);
        let url = format!("{FABRIC_META_URL}/versions/loader/{game}/{loader}/profile/json");
        let cache = self
            .paths
            .version_json(&format!("fabric-loader-{loader}-{game}"))?;
        // Il profilo di una coppia loader/gioco fissata non cambia: la copia
        // salvata, se leggibile, evita una richiesta a ogni avvio.
        let cached = cache
            .is_file()
            .then(|| read_json::<VersionJson>(&cache).ok());
        let profile = match cached.flatten() {
            Some(profile) => profile,
            None => self.fetch_json_cached(&url, &cache).await?,
        };
        if profile.inherits_from.as_deref() != Some(game) {
            return Err(InstallerError::UnsupportedVersion {
                version: profile.id,
                reason: format!("loader profile does not inherit from {game}"),
            });
        }
        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_spec_toml_round_trip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Holder {
            loader: LoaderSpec,
        }
        let holder = Holder {
            loader: LoaderSpec {
                kind: LoaderKind::Fabric,
                version: "0.20.1".into(),
            },
        };
        let text = toml::to_string(&holder).unwrap();
        assert!(text.contains("kind = \"fabric\""), "{text}");
        assert_eq!(toml::from_str::<Holder>(&text).unwrap(), holder);
    }

    #[test]
    fn parses_fabric_loader_list() {
        let json = r#"[{"loader":{"separator":".","build":1,"maven":"net.fabricmc:fabric-loader:0.20.1","version":"0.20.1","stable":true},
                       "intermediary":{"maven":"net.fabricmc:intermediary:0.0.0","version":"0.0.0","stable":true}}]"#;
        let entries: Vec<FabricLoaderEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries[0].loader.version, "0.20.1");
        assert!(entries[0].loader.stable);
    }
}
