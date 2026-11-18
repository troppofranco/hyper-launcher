//! Istanze isolate (ognuna con la propria cartella di gioco e `instance.toml`),
//! costruzione del comando di avvio JVM e pipeline completa di avvio
//! riusabile da CLI e UI.

pub mod console;
mod content;
mod launch;
mod prepare;
pub mod servers;
pub mod skins;

use std::path::{Path, PathBuf};

use hyper_core::{Config, CoreError, Paths, fs};
use hyper_installer::LoaderSpec;
use serde::{Deserialize, Serialize};

pub use content::{ContentDir, ContentFile, DISABLED, World};
pub use launch::{LaunchCommand, LaunchOptions, build_command, prepare_natives};
pub use prepare::{PreparedLaunch, RunningGame, install, prepare_launch, resolve_java};

pub use hyper_core::MIN_MEMORY_MB;

/// Immagine scelta per l'istanza (accanto a `instance.toml`).
const ICON_FILE: &str = "icon.png";
/// L'interfaccia la riduce a 256×256 prima di salvarla: basta e avanza.
const MAX_ICON_BYTES: usize = 1024 * 1024;

/// Storico delle partite di un'istanza (accanto a `instance.toml`).
const SESSIONS_FILE: &str = "sessions.json";
/// Sessioni conservate per istanza: abbastanza per mesi di statistiche.
const MAX_SESSIONS: usize = 1000;

/// Una partita: inizio e fine in secondi dall'epoch Unix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaySession {
    pub start: u64,
    pub end: u64,
}

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

    #[error("invalid image: {0}")]
    InvalidImage(String),

    #[error("{0:?} is not a valid content file name")]
    InvalidContentFile(String),

    #[error("{0} already exists: remove one of the two copies")]
    ContentConflict(String),

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
    /// Mod loader; assente = vanilla.
    #[serde(default)]
    pub loader: Option<LoaderSpec>,
    /// Secondi dall'epoch Unix.
    pub created: u64,
    /// Fine dell'ultima sessione di gioco (secondi dall'epoch Unix).
    #[serde(default)]
    pub last_played: Option<u64>,
    /// Tempo di gioco totale in secondi.
    #[serde(default)]
    pub playtime_secs: u64,
    /// Usa le impostazioni condivise (se la sincronizzazione è attiva).
    #[serde(default = "default_true")]
    pub sync_settings: bool,
}

fn default_true() -> bool {
    true
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

    /// Registra una sessione di gioco terminata: totale e ultima partita in
    /// `instance.toml`, la sessione nello storico (per le statistiche).
    pub fn record_session(&mut self, started: u64, ended: u64) -> Result<()> {
        self.config.playtime_secs += ended.saturating_sub(started);
        self.config.last_played = Some(ended);
        self.save()?;
        let mut sessions = self.sessions();
        sessions.push(PlaySession {
            start: started,
            end: ended,
        });
        let excess = sessions.len().saturating_sub(MAX_SESSIONS);
        sessions.drain(..excess);
        let json = serde_json::to_vec(&sessions).expect("sessions serialize");
        Ok(fs::write_atomic(&self.dir.join(SESSIONS_FILE), &json)?)
    }

    /// Immagine dell'istanza (PNG), se è stata scelta.
    pub fn icon(&self) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(ICON_FILE)).ok()
    }

    /// Imposta (PNG) o toglie (`None`) l'immagine dell'istanza.
    pub fn set_icon(&self, png: Option<&[u8]>) -> Result<()> {
        let path = self.dir.join(ICON_FILE);
        let Some(png) = png else {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Err(CoreError::io(&path, e).into())
                }
                _ => Ok(()),
            };
        };
        if png.len() > MAX_ICON_BYTES || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(InstanceError::InvalidImage(
                "the image must be a PNG of at most 1 MB".into(),
            ));
        }
        Ok(fs::write_atomic(&path, png)?)
    }

    /// Storico delle sessioni di gioco (dalla più vecchia); vuoto se manca
    /// o è illeggibile, perché serve solo alle statistiche.
    pub fn sessions(&self) -> Vec<PlaySession> {
        std::fs::read(self.dir.join(SESSIONS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Rilegge da disco la stessa istanza.
    pub fn reload(instance: &Instance) -> Result<Instance> {
        Self::load_from(instance.dir.clone())?
            .ok_or_else(|| InstanceError::NotFound(instance.config.name.clone()))
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
    pub fn create(
        &self,
        name: &str,
        version: &str,
        loader: Option<LoaderSpec>,
    ) -> Result<Instance> {
        let dir = self.paths.instance_dir(name)?;
        if dir.exists() {
            return Err(InstanceError::AlreadyExists(name.to_owned()));
        }
        let instance = Instance {
            config: InstanceConfig {
                name: name.to_owned(),
                version: version.to_owned(),
                memory_mb: None,
                java_path: None,
                jvm_args: Vec::new(),
                loader,
                created: hyper_core::unix_now(),
                last_played: None,
                playtime_secs: 0,
                sync_settings: true,
            },
            dir,
        };
        fs::create_dir_all(&instance.game_dir())?;
        instance.save()?;
        Ok(instance)
    }

    /// Come [`Self::create`], ma con nome vuoto ne sceglie uno automatico
    /// ("Fabric 26.3", "Minecraft 26.3", poi "… (2)", "… (3)" se occupato).
    pub fn create_named(
        &self,
        name: &str,
        version: &str,
        loader: Option<LoaderSpec>,
    ) -> Result<Instance> {
        let name = match name.trim() {
            "" => self.unique_name(&match &loader {
                Some(l) => format!("{} {version}", l.kind.display_name()),
                None => format!("Minecraft {version}"),
            })?,
            name => name.to_owned(),
        };
        self.create(&name, version, loader)
    }

    /// `base`, oppure `base (2)`, `base (3)`… se una cartella con quel nome
    /// (ignorando le maiuscole, come Windows) esiste già.
    pub fn unique_name(&self, base: &str) -> Result<String> {
        let taken = |name: &str| -> Result<bool> {
            let dir = self.paths.instance_dir(name)?;
            Ok(dir.exists())
        };
        let mut name = base.to_owned();
        let mut n = 2;
        while taken(&name)? {
            name = format!("{base} ({n})");
            n += 1;
        }
        Ok(name)
    }

    /// Elimina per sempre un'istanza (mondi compresi). Rifiuta se è in uso
    /// da questo o da un altro processo.
    pub fn delete(&self, name: &str) -> Result<()> {
        let instance = self.load(name)?;
        let lock = instance.try_lock()?;
        // Il lock va rilasciato prima: su Windows un file aperto non si cancella.
        drop(lock);
        fs::remove_dir_all_if_exists(&instance.dir)?;
        Ok(())
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

        let created = manager.create("survival", "1.21.4", None).unwrap();
        assert!(created.game_dir().is_dir());
        assert!(created.dir.join("instance.toml").is_file());
        manager.create("alpha", "1.20.1", None).unwrap();
        std::fs::create_dir_all(dir.path().join("instances").join("not-an-instance")).unwrap();

        assert!(matches!(
            manager.create("survival", "1.21.4", None),
            Err(InstanceError::AlreadyExists(_))
        ));
        assert!(manager.create("../escape", "1.21.4", None).is_err());
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
            .create("m", "1.21.4", None)
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

    #[test]
    fn automatic_names_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let manager = InstanceManager::new(Paths::new(dir.path()));
        let fabric = LoaderSpec {
            kind: hyper_installer::LoaderKind::Fabric,
            version: "0.20.1".into(),
        };
        let a = manager
            .create_named("", "26.3", Some(fabric.clone()))
            .unwrap();
        let b = manager.create_named("  ", "26.3", Some(fabric)).unwrap();
        let c = manager.create_named("", "26.3", None).unwrap();
        assert_eq!(a.config.name, "Fabric 26.3");
        assert_eq!(b.config.name, "Fabric 26.3 (2)");
        assert_eq!(c.config.name, "Minecraft 26.3");

        let lock = a.try_lock().unwrap();
        assert!(matches!(
            manager.delete("Fabric 26.3"),
            Err(InstanceError::Busy(_))
        ));
        drop(lock);
        manager.delete("Fabric 26.3").unwrap();
        assert!(!a.dir.exists());
        assert!(matches!(
            manager.load("Fabric 26.3"),
            Err(InstanceError::NotFound(_))
        ));
    }

    #[test]
    fn records_sessions_for_statistics() {
        let dir = tempfile::tempdir().unwrap();
        let mut instance = InstanceManager::new(Paths::new(dir.path()))
            .create("s", "26.3", None)
            .unwrap();
        assert!(instance.sessions().is_empty());
        instance.record_session(1000, 1600).unwrap();
        instance.record_session(5000, 5060).unwrap();
        assert_eq!(instance.config.playtime_secs, 660);
        assert_eq!(instance.config.last_played, Some(5060));
        let sessions = instance.sessions();
        assert_eq!(sessions.len(), 2);
        assert_eq!(
            sessions[0],
            PlaySession {
                start: 1000,
                end: 1600
            }
        );
        // Un file rovinato non blocca il gioco: si riparte da zero.
        std::fs::write(instance.dir.join(SESSIONS_FILE), b"{oops").unwrap();
        assert!(instance.sessions().is_empty());
        instance.record_session(7000, 7100).unwrap();
        assert_eq!(instance.sessions().len(), 1);
    }
}
