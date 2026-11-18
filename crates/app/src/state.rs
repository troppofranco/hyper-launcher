//! Stato condiviso dell'app e conversione degli errori per la UI.

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use hyper_content::{CurseForge, Modrinth};
use hyper_core::{Config, Paths};
use hyper_installer::Installer;
use hyper_instances::InstanceManager;

/// Risultato di un comando: in caso di errore la UI riceve il messaggio.
pub type CmdResult<T> = Result<T, String>;

/// `.cmd()` trasforma qualunque errore in un messaggio leggibile per la UI,
/// con l'intera catena di cause (`{:#}`).
pub trait OrCmd<T> {
    fn cmd(self) -> CmdResult<T>;
}

impl<T, E: std::fmt::Display> OrCmd<T> for Result<T, E> {
    fn cmd(self) -> CmdResult<T> {
        self.map_err(|e| format!("{e:#}"))
    }
}

pub struct AppState {
    pub paths: Paths,
    /// Client del catalogo, condiviso (connessioni riusate).
    pub modrinth: Modrinth,
    /// Client CurseForge: c'è solo se la build ha la chiave API.
    pub curseforge: Option<CurseForge>,
    config: Mutex<Config>,
    /// Un solo client HTTP per tutta l'app (connessioni riusate); ricreato
    /// solo se cambia il numero di download in parallelo.
    installer: Mutex<Installer>,
    /// Istanze in installazione o in gioco.
    busy: Mutex<HashSet<String>>,
    /// Sfondo trasparente attivo (effetto della finestra applicato da Windows).
    window_effect: std::sync::atomic::AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    pub fn load() -> anyhow::Result<Self> {
        let paths = Paths::from_env()?;
        let config = Config::load(&paths)?;
        let installer = Installer::new(paths.clone(), config.download_concurrency)?;
        Ok(Self {
            modrinth: Modrinth::new()?,
            curseforge: CurseForge::new(),
            paths,
            config: Mutex::new(config),
            installer: Mutex::new(installer),
            busy: Mutex::new(HashSet::new()),
            window_effect: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn config(&self) -> Config {
        lock(&self.config).clone()
    }

    pub fn save_config(&self, config: Config) -> anyhow::Result<()> {
        config.save(&self.paths)?;
        let mut current = lock(&self.config);
        if current.download_concurrency != config.download_concurrency {
            *lock(&self.installer) =
                Installer::new(self.paths.clone(), config.download_concurrency)?;
        }
        *current = config;
        Ok(())
    }

    /// Copia economica: il client HTTP interno è condiviso.
    pub fn installer(&self) -> Installer {
        lock(&self.installer).clone()
    }

    pub fn instances(&self) -> InstanceManager {
        InstanceManager::new(self.paths.clone())
    }

    /// Segna un'istanza come occupata; `false` se lo era già.
    pub fn try_mark_busy(&self, name: &str) -> bool {
        lock(&self.busy).insert(name.to_owned())
    }

    pub fn clear_busy(&self, name: &str) {
        lock(&self.busy).remove(name);
    }

    pub fn is_busy(&self, name: &str) -> bool {
        lock(&self.busy).contains(name)
    }

    pub fn any_busy(&self) -> bool {
        !lock(&self.busy).is_empty()
    }

    pub fn set_window_effect(&self, active: bool) {
        self.window_effect
            .store(active, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn window_effect(&self) -> bool {
        self.window_effect
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
