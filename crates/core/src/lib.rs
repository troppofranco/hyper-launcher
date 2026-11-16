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
