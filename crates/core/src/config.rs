use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{CoreError, Paths, fs};

/// Configurazione globale del launcher (`config.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    /// Memoria massima (`-Xmx`) per le istanze che non la specificano.
    pub default_memory_mb: u32,
    /// Java da usare al posto del runtime Mojang scaricato automaticamente.
    pub java_path: Option<PathBuf>,
    /// Numero massimo di download simultanei.
    pub download_concurrency: usize,
    /// Application (client) ID dell'app Azure usata per il login Microsoft.
    /// Non è un segreto. Sovrascrivibile con `HYPER_MSA_CLIENT_ID`.
    pub msa_client_id: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_memory_mb: 4096,
            java_path: None,
            download_concurrency: 16,
            msa_client_id: None,
        }
    }
}

impl Config {
    pub const MSA_CLIENT_ID_ENV: &str = "HYPER_MSA_CLIENT_ID";

    /// Client ID Microsoft effettivo: variabile d'ambiente, poi config.
    pub fn effective_msa_client_id(&self) -> Option<String> {
        std::env::var(Self::MSA_CLIENT_ID_ENV)
            .ok()
            .or_else(|| self.msa_client_id.clone())
            .map(|id| id.trim().to_owned())
            .filter(|id| !id.is_empty())
    }

    pub fn load(paths: &Paths) -> Result<Self, CoreError> {
        fs::read_toml_or_default(&paths.config_file())
    }

    pub fn save(&self, paths: &Paths) -> Result<(), CoreError> {
        fs::write_toml(&paths.config_file(), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let mut config = Config::load(&paths).unwrap();
        assert_eq!(config, Config::default());

        config.default_memory_mb = 6144;
        config.save(&paths).unwrap();
        assert_eq!(Config::load(&paths).unwrap(), config);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::write(paths.config_file(), "default_memory_mb = 2048\n").unwrap();
        let config = Config::load(&paths).unwrap();
        assert_eq!(config.default_memory_mb, 2048);
        assert_eq!(config.download_concurrency, 16);
    }
}
