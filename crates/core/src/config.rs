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
    /// Colore accento dell'interfaccia (`#rrggbb`).
    pub accent: String,
    /// Riduce a icona la finestra del launcher quando il gioco parte.
    pub minimize_on_launch: bool,
    /// Impostazioni di gioco condivise tra le istanze.
    pub sync: SyncConfig,
    /// Mostra la finestra di console di Java accanto al gioco (Windows).
    pub show_console: bool,
    /// Tema dell'interfaccia: `dark` o `light`.
    pub theme: String,
    /// Lingua dell'interfaccia (`it`, `en`).
    pub language: String,
    /// Configurazione iniziale completata (account, aspetto, lingua).
    pub setup_done: bool,
}

/// Quali impostazioni di `options.txt` condividere tra le istanze.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncConfig {
    /// Spenta finché l'utente non la attiva: cambia le impostazioni di
    /// tutte le istanze.
    pub enabled: bool,
    pub keybinds: bool,
    pub video: bool,
    pub audio: bool,
    pub controls: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keybinds: true,
            video: true,
            audio: true,
            controls: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_memory_mb: 4096,
            java_path: None,
            download_concurrency: 16,
            msa_client_id: None,
            accent: "#6e5ae6".into(),
            minimize_on_launch: true,
            sync: SyncConfig::default(),
            show_console: false,
            theme: "dark".into(),
            language: "it".into(),
            setup_done: false,
        }
    }
}

/// Sotto questa soglia la JVM non riesce nemmeno ad avviare il gioco.
pub const MIN_MEMORY_MB: u32 = 512;

impl Config {
    /// Controlla i valori prima di salvarli (anche se scritti a mano).
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.default_memory_mb < MIN_MEMORY_MB {
            return Err(CoreError::InvalidConfig(format!(
                "default memory must be at least {MIN_MEMORY_MB} MB"
            )));
        }
        let hex = self.accent.strip_prefix('#').unwrap_or_default();
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CoreError::InvalidConfig(format!(
                "accent color {:?} is not #rrggbb",
                self.accent
            )));
        }
        if !Self::THEMES.contains(&self.theme.as_str()) {
            return Err(CoreError::InvalidConfig(format!(
                "unknown theme {:?}",
                self.theme
            )));
        }
        if !Self::LANGUAGES.contains(&self.language.as_str()) {
            return Err(CoreError::InvalidConfig(format!(
                "unknown language {:?}",
                self.language
            )));
        }
        Ok(())
    }

    /// Temi e lingue che l'interfaccia conosce.
    pub const THEMES: [&str; 2] = ["dark", "light"];
    pub const LANGUAGES: [&str; 2] = ["it", "en"];

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
        self.validate()?;
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
