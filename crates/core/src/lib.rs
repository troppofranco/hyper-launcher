//! Tipi comuni a tutti i crate di Hyper Launcher: layout delle cartelle,
//! configurazione globale, informazioni sulla piattaforma e utilità di
//! filesystem sicure (anti path traversal, scritture atomiche).

mod config;
mod error;
pub mod fs;
mod paths;
pub mod platform;
mod safe_path;

pub use config::Config;
pub use error::CoreError;
pub use paths::Paths;
pub use safe_path::{safe_component, safe_join};

/// Nome del launcher passato al gioco (`${launcher_name}`).
pub const LAUNCHER_NAME: &str = "hyper-launcher";
/// Versione del launcher passata al gioco (`${launcher_version}`).
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// User-Agent di tutte le richieste HTTP del launcher.
pub const USER_AGENT: &str = concat!("HyperLauncher/", env!("CARGO_PKG_VERSION"));

/// Secondi dall'epoch Unix (0 se l'orologio è prima del 1970).
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
