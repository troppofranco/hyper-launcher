//! Layout della cartella dati del launcher.
//!
//! ```text
//! <root>/config.toml  accounts.toml
//! <root>/versions/<id>/<id>.json  <id>.jar        cache condivisa
//! <root>/libraries/<percorso maven>               cache condivisa
//! <root>/assets/indexes/<id>.json  objects/ab/<hash>  log_configs/
//! <root>/runtimes/<component>/                    runtime Java Mojang
//! <root>/instances/<nome>/                        istanze isolate
//! ```
//!
//! I metodi che accettano un identificatore (versione, componente, istanza)
//! lo validano con [`safe_component`], perché può arrivare da un manifest
//! remoto o dall'utente.

use std::path::{Path, PathBuf};

use crate::{CoreError, safe_component};

#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// Variabile d'ambiente che sostituisce la cartella dati predefinita.
    pub const HOME_ENV: &str = "HYPER_LAUNCHER_HOME";

    /// `HYPER_LAUNCHER_HOME` se impostata, altrimenti la cartella dati
    /// dell'utente (`%APPDATA%\HyperLauncher` su Windows).
    pub fn from_env() -> Result<Self, CoreError> {
        if let Some(home) = std::env::var_os(Self::HOME_ENV).filter(|v| !v.is_empty()) {
            return Ok(Self::new(home));
        }
        let base = directories::BaseDirs::new().ok_or(CoreError::NoDataDir)?;
        Ok(Self::new(base.data_dir().join("HyperLauncher")))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.toml")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn version_manifest_cache(&self) -> PathBuf {
        self.versions_dir().join("version_manifest_v2.json")
    }

    pub fn version_dir(&self, id: &str) -> Result<PathBuf, CoreError> {
        Ok(self.versions_dir().join(safe_component(id)?))
    }

    pub fn version_json(&self, id: &str) -> Result<PathBuf, CoreError> {
        Ok(self.version_dir(id)?.join(format!("{id}.json")))
    }

    pub fn version_jar(&self, id: &str) -> Result<PathBuf, CoreError> {
        Ok(self.version_dir(id)?.join(format!("{id}.jar")))
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn asset_index(&self, id: &str) -> Result<PathBuf, CoreError> {
        Ok(self
            .assets_dir()
            .join("indexes")
            .join(format!("{}.json", safe_component(id)?)))
    }

    pub fn asset_objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    pub fn log_config(&self, file_id: &str) -> Result<PathBuf, CoreError> {
        Ok(self
            .assets_dir()
            .join("log_configs")
            .join(safe_component(file_id)?))
    }

    pub fn runtimes_dir(&self) -> PathBuf {
        self.root.join("runtimes")
    }

    pub fn runtime_dir(&self, component: &str) -> Result<PathBuf, CoreError> {
        Ok(self.runtimes_dir().join(safe_component(component)?))
    }

    pub fn runtime_manifest(&self, component: &str) -> Result<PathBuf, CoreError> {
        Ok(self
            .runtimes_dir()
            .join(format!("{}.manifest.json", safe_component(component)?)))
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn instance_dir(&self, name: &str) -> Result<PathBuf, CoreError> {
        Ok(self.instances_dir().join(safe_component(name)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_identifiers() {
        let paths = Paths::new("root");
        assert!(paths.version_dir("../x").is_err());
        assert!(paths.instance_dir("a/b").is_err());
        assert!(paths.runtime_dir("..").is_err());
        assert_eq!(
            paths.version_jar("1.21.4").unwrap(),
            Path::new("root")
                .join("versions")
                .join("1.21.4")
                .join("1.21.4.jar")
        );
    }
}
