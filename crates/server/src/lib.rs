//! Server Minecraft ospitati dal launcher.
//!
//! - [`config`]: i server creati (`servers/<nome>.server.toml`) e il loro
//!   elenco.
//! - [`install`]: download del server (Vanilla da Mojang con SHA1, Paper
//!   dall'API di PaperMC con SHA256) e di Java.
//! - [`host`]: il processo in background che possiede i server accesi, così
//!   restano attivi a launcher chiuso; [`client`] ci parla da launcher e CLI
//!   (TCP solo su 127.0.0.1, con un token).
//! - [`properties`], [`files`], [`players`]: impostazioni, file e giocatori,
//!   sempre dentro la cartella del server (path traversal escluso).

pub mod autostart;
pub mod client;
pub mod config;
mod error;
pub mod files;
pub mod host;
pub mod install;
pub mod players;
pub mod plugins;
pub mod properties;
pub mod protocol;

pub use client::HostClient;
pub use config::{ServerConfig, ServerKind, ServerManager};
pub use error::{Result, ServerError};
