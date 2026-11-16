//! Istanze isolate (ognuna con la propria cartella di gioco e `instance.toml`),
//! costruzione del comando di avvio JVM e pipeline completa di avvio
//! riusabile da CLI e UI.

mod launch;
mod prepare;

use std::path::{Path, PathBuf};

use hyper_core::{Config, CoreError, Paths, fs};
use serde::{Deserialize, Serialize};

pub use launch::{LaunchCommand, LaunchOptions, build_command, prepare_natives};
pub use prepare::{PreparedLaunch, RunningGame, install, prepare_launch, resolve_java};

/// Sotto questa soglia la JVM non riesce nemmeno ad avviare il gioco.
pub const MIN_MEMORY_MB: u32 = 512;

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error(transparent)]
    Core(#[from] CoreError),

    /// In `Box` perché `InstallerError` è grande e renderebbe pesante ogni `Result`.
    #[error(transparent)]
    Installer(Box<hyper_installer::InstallerError>),

    #[error("instance {0:?} already exists")]
    AlreadyExists(String),

    #[error("instance {0:?} not found")]
    NotFound(String),

    #[error("instance {0:?} is already in use (is the game still running?)")]
    Busy(String),

    #[error("memory too low ({mb} MB): at least {MIN_MEMORY_MB} MB are required")]
    MemoryTooLow { mb: u32 },

    #[error("Java not found: {0}")]
    JavaNotFound(PathBuf),

    #[error("game process error ({program}): {source}")]
    Process {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl From<hyper_installer::InstallerError> for InstanceError {
    fn from(e: hyper_installer::InstallerError) -> Self {
        Self::Installer(Box::new(e))
    }
}

pub type Result<T, E = InstanceError> = std::result::Result<T, E>;

/// Contenuto di `instance.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub name: String,
    /// Id della versione Minecraft (es. `1.21.4`).
    pub version: String,
    /// `-Xmx` in MB; se assente si usa il default globale.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    /// Java specifico per questa istanza.
    #[serde(default)]
    pub java_path: Option<PathBuf>,
    /// Argomenti JVM aggiuntivi.
    #[serde(default)]
    pub jvm_args: Vec<String>,
    /// Secondi dall'epoch Unix.
    pub created: u64,
}

impl InstanceConfig {
    /// Memoria effettiva (istanza, altrimenti default globale), validata.
    pub fn effective_memory_mb(&self, config: &Config) -> Result<u32> {
        let mb = self.memory_mb.unwrap_or(config.default_memory_mb);
        if mb < MIN_MEMORY_MB {
            return Err(InstanceError::MemoryTooLow { mb });
        }
        Ok(mb)
    }
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub config: InstanceConfig,
    pub dir: PathBuf,
}

impl Instance {
    const CONFIG_FILE: &str = "instance.toml";

    /// Cartella `.minecraft` dell'istanza (mondi, opzioni, mod, log).
    pub fn game_dir(&self) -> PathBuf {
        self.dir.join("minecraft")
    }

    /// Native estratte, rigenerate a ogni avvio.
    pub fn natives_dir(&self) -> PathBuf {
        self.dir.join("natives")
    }

    fn config_path(dir: &Path) -> PathBuf {
        dir.join(Self::CONFIG_FILE)
    }

    /// Lock esclusivo sull'istanza, rilasciato quando il `File` restituito
    /// viene chiuso. Impedisce due avvii simultanei della stessa istanza.
    pub fn try_lock(&self) -> Result<std::fs::File> {
        let path = self.dir.join(".lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| CoreError::io(&path, e))?;
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => {
                Err(InstanceError::Busy(self.config.name.clone()))
            }
            Err(std::fs::TryLockError::Error(e)) => Err(CoreError::io(&path, e).into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        Ok(fs::write_toml(&Self::config_path(&self.dir), &self.config)?)
    }

    /// `None` se la cartella non contiene un `instance.toml`.
    fn load_from(dir: PathBuf) -> Result<Option<Self>> {
        let path = Self::config_path(&dir);
        if !path.is_file() {
            return Ok(None);
        }
        let config = fs::read_toml(&path)?;
        Ok(Some(Self { config, dir }))
    }
}

#[derive(Debug, Clone)]
pub struct InstanceManager {
    paths: Paths,
}

impl InstanceManager {
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    /// Crea cartella isolata e `instance.toml`. Il nome deve essere un nome
    /// di cartella sicuro (validato con `safe_component`).
    pub fn create(&self, name: &str, version: &str) -> Result<Instance> {
        let dir = self.paths.instance_dir(name)?;
        if dir.exists() {
            return Err(InstanceError::AlreadyExists(name.to_owned()));
        }
        let created = hyper_core::unix_now();
        let instance = Instance {
            config: InstanceConfig {
                name: name.to_owned(),
                version: version.to_owned(),
                memory_mb: None,
                java_path: None,
                jvm_args: Vec::new(),
                created,
            },
            dir,
        };
        fs::create_dir_all(&instance.game_dir())?;
        instance.save()?;
        Ok(instance)
    }

    pub fn load(&self, name: &str) -> Result<Instance> {
        Instance::load_from(self.paths.instance_dir(name)?)?
            .ok_or_else(|| InstanceError::NotFound(name.to_owned()))
    }

    /// Tutte le istanze valide, ordinate per nome. Le cartelle con un
    /// `instance.toml` illeggibile vengono saltate con un warning.
    pub fn list(&self) -> Result<Vec<Instance>> {
        let root = self.paths.instances_dir();
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(CoreError::io(&root, e).into()),
        };
        let mut instances = Vec::new();
        for entry in entries {
            let dir = entry.map_err(|e| CoreError::io(&root, e))?.path();
            match Instance::load_from(dir) {
                Ok(Some(instance)) => instances.push(instance),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "skipping unreadable instance"),
            }
        }
        instances.sort_by(|a, b| a.config.name.cmp(&b.config.name));
        Ok(instances)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_load_list() {
        let dir = tempfile::tempdir().unwrap();
        let manager = InstanceManager::new(Paths::new(dir.path()));

        let created = manager.create("survival", "1.21.4").unwrap();
        assert!(created.game_dir().is_dir());
        assert!(created.dir.join("instance.toml").is_file());
        manager.create("alpha", "1.20.1").unwrap();
        std::fs::create_dir_all(dir.path().join("instances").join("not-an-instance")).unwrap();

        assert!(matches!(
            manager.create("survival", "1.21.4"),
            Err(InstanceError::AlreadyExists(_))
        ));
        assert!(manager.create("../escape", "1.21.4").is_err());
        assert!(matches!(
            manager.load("missing"),
            Err(InstanceError::NotFound(_))
        ));

        let loaded = manager.load("survival").unwrap();
        assert_eq!(loaded.config, created.config);
        let names: Vec<_> = manager
            .list()
            .unwrap()
            .into_iter()
            .map(|i| i.config.name)
            .collect();
        assert_eq!(names, ["alpha", "survival"]);

        let lock = loaded.try_lock().unwrap();
        assert!(matches!(loaded.try_lock(), Err(InstanceError::Busy(_))));
        drop(lock);
        assert!(loaded.try_lock().is_ok());
    }

    #[test]
    fn memory_validation() {
        let dir = tempfile::tempdir().unwrap();
        let mut instance = InstanceManager::new(Paths::new(dir.path()))
            .create("m", "1.21.4")
            .unwrap();
        let config = Config::default();
        assert_eq!(
            instance.config.effective_memory_mb(&config).unwrap(),
            config.default_memory_mb
        );
        instance.config.memory_mb = Some(128);
        assert!(matches!(
            instance.config.effective_memory_mb(&config),
            Err(InstanceError::MemoryTooLow { mb: 128 })
        ));
    }
}
