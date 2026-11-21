//! Installazioni di Minecraft Bedrock su Windows (versione GDK, dalla
//! 1.21.120).
//!
//! Bedrock ha un solo gioco installato (Microsoft Store) e un solo posto per
//! i dati: `%APPDATA%\Minecraft Bedrock\Users\{Shared,<utente>}\games\com.mojang`.
//! Un'"installazione" del launcher è quindi un profilo: le cartelle gestite
//! ([`FOLDERS`]) del profilo attivo stanno dentro il gioco, quelle degli
//! altri nella cartella dell'istanza (`<istanza>/bedrock/<ambito>/<cartella>`).
//! Prima di avviare un profilo diverso le cartelle si scambiano con dei
//! `rename` ([`swap`]), registrati in un diario così un'interruzione a metà
//! si completa al riavvio del launcher.

mod game;
mod packs;
mod swap;

use std::path::PathBuf;

pub use game::{
    DataDirs, Game, LAUNCH_URI, running, wait_for_exit, wait_for_helper, wait_for_start,
};
pub use packs::{Imported, Pack, PackKind, delete_pack, import, list};
pub use swap::{State, recover, set_active, swap};

/// Ambito di una cartella di `com.mojang`: quella condivisa da tutti gli
/// utenti Windows/Xbox del PC o quella del profilo Xbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Shared,
    User,
}

impl Scope {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::User => "user",
        }
    }
}

/// Le cartelle che appartengono a un'installazione. I pacchetti stanno di
/// norma in `Shared`, ma il gioco può importarli anche nel profilo: si
/// gestiscono entrambi. Il resto di `com.mojang` (acquisti del Marketplace,
/// cache, pacchetti in sviluppo) resta del gioco.
pub const FOLDERS: &[(Scope, &str)] = &[
    (Scope::Shared, "resource_packs"),
    (Scope::Shared, "behavior_packs"),
    (Scope::Shared, "skin_packs"),
    // Il gioco ha anche una cartella di mondi condivisa tra i profili.
    (Scope::Shared, "minecraftWorlds"),
    (Scope::User, "resource_packs"),
    (Scope::User, "behavior_packs"),
    (Scope::User, "skin_packs"),
    (Scope::User, "minecraftWorlds"),
    (Scope::User, "world_templates"),
    // Di `minecraftpe` solo impostazioni e server aggiunti: il resto (id del
    // client, cache del catalogo, telemetria) è del gioco.
    (Scope::User, "minecraftpe/options.txt"),
    (Scope::User, "minecraftpe/external_servers.txt"),
];

/// Dove stanno adesso i dati di un'installazione: nel gioco se è quella
/// attiva, altrimenti nella sua cartella del launcher.
#[derive(Debug, Clone)]
pub struct Location {
    /// `None` per l'attiva: nel gioco.
    stored: Option<PathBuf>,
    dirs: DataDirs,
}

impl Location {
    /// `storage` è `<istanza>/bedrock`.
    pub fn new(dirs: DataDirs, storage: PathBuf, active: bool) -> Self {
        Self {
            stored: (!active).then_some(storage),
            dirs,
        }
    }

    /// Percorso di una cartella gestita; `None` se l'installazione è attiva
    /// e il gioco non ha ancora creato la cartella del profilo Xbox.
    pub fn folder(&self, scope: Scope, name: &str) -> Option<PathBuf> {
        match &self.stored {
            Some(storage) => Some(storage.join(scope.dir_name()).join(name)),
            None => self.dirs.game_folder(scope, name),
        }
    }

    pub fn is_active(&self) -> bool {
        self.stored.is_none()
    }

    /// `minecraftpe/options.txt`: impostazioni e tasti.
    pub fn options_file(&self) -> Option<PathBuf> {
        self.folder(Scope::User, "minecraftpe/options.txt")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BedrockError {
    #[error(transparent)]
    Core(#[from] hyper_core::CoreError),

    /// In una `Box`: l'errore dell'installer è grande e renderebbe pesante
    /// ogni `Result` del crate.
    #[error(transparent)]
    Installer(Box<hyper_installer::InstallerError>),

    #[error("Minecraft Bedrock non è installato")]
    NotInstalled,

    #[error("chiudi Minecraft Bedrock e riprova")]
    GameRunning,

    #[error(
        "Bedrock non ha ancora creato le cartelle del tuo profilo: aprilo una volta (e accedi con il tuo account Microsoft), poi riprova"
    )]
    NoProfile,

    #[error(
        "Minecraft Bedrock non si è aperto: se l'hai appena chiuso, aspetta un minuto e riprova"
    )]
    DidNotStart,

    #[error(
        "{0} esiste già: per non perdere dati lo scambio si è fermato. Controlla la cartella e riprova"
    )]
    Conflict(PathBuf),

    #[error(
        "le cartelle del launcher e di Bedrock sono su dischi diversi: lo scambio delle installazioni non è possibile"
    )]
    CrossDevice,

    #[error("{0}")]
    Invalid(String),
}

impl From<hyper_installer::InstallerError> for BedrockError {
    fn from(e: hyper_installer::InstallerError) -> Self {
        Self::Installer(Box::new(e))
    }
}

pub type Result<T, E = BedrockError> = std::result::Result<T, E>;
