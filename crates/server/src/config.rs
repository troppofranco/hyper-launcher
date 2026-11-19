//! I server creati: configurazione del launcher accanto alla cartella dei
//! file del server (`servers/<nome>/` e `servers/<nome>.server.toml`).

use std::path::PathBuf;

use hyper_core::{Paths, fs, safe_component};
use serde::{Deserialize, Serialize};

use crate::{Result, ServerError};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerKind {
    Vanilla,
    Paper,
}

impl ServerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Paper => "paper",
        }
    }
}

/// Configurazione di un server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerConfig {
    pub name: String,
    pub kind: ServerKind,
    /// Versione di Minecraft.
    pub version: String,
    /// Build di Paper (nessuna per Vanilla).
    #[serde(default)]
    pub build: Option<u32>,
    pub memory_mb: u32,
    pub port: u16,
    /// Resta acceso anche quando il launcher viene chiuso.
    #[serde(default)]
    pub keep_running: bool,
    /// Parte all'accesso a Windows (richiede `keep_running`).
    #[serde(default)]
    pub autostart: bool,
    pub created: u64,
    /// Componente del runtime Java Mojang da usare.
    pub java_component: String,
    /// Tunnel playit.gg creato dal launcher (accesso da internet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playit_tunnel: Option<uuid::Uuid>,
    /// Indirizzo pubblico del tunnel (es. `nome.tun.ply.gg`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playit_address: Option<String>,
}

/// Memoria ammessa per un server, in MB.
pub const MEMORY_MB: std::ops::RangeInclusive<u32> = 512..=65536;
/// Porte ammesse (sotto 1024 servono privilegi).
pub const PORTS: std::ops::RangeInclusive<u16> = 1024..=65535;
/// File del server Minecraft dentro la sua cartella.
pub const JAR: &str = "server.jar";
/// Immagine del server nella lista dei server di Minecraft.
pub const ICON: &str = "server-icon.png";
/// Lato dell'immagine richiesto da Minecraft.
pub const ICON_SIDE: u32 = 64;
const MAX_ICON_BYTES: u64 = 256 * 1024;

/// Controlla che `bytes` sia un PNG 64×64 (intestazione e blocco IHDR).
fn check_icon(bytes: &[u8]) -> Result<()> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    let bad = |msg: &str| Err(ServerError::Invalid(msg.into()));
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return bad("the image is too large");
    }
    if bytes.len() < 24 || !bytes.starts_with(SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return bad("the server image must be a PNG");
    }
    let side =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    if side(16) != ICON_SIDE || side(20) != ICON_SIDE {
        return bad("the server image must be 64×64 pixels");
    }
    Ok(())
}

impl ServerConfig {
    pub fn validate(&self) -> Result<()> {
        self.validate_settings()?;
        safe_component(&self.java_component)?;
        Ok(())
    }

    /// Nome, memoria e porta: quello che sceglie l'utente.
    pub fn validate_settings(&self) -> Result<()> {
        validate_name(&self.name)?;
        if !MEMORY_MB.contains(&self.memory_mb) {
            return Err(ServerError::Invalid(
                "memory must be between 512 MB and 64 GB".into(),
            ));
        }
        if !PORTS.contains(&self.port) {
            return Err(ServerError::Invalid(
                "port must be between 1024 and 65535".into(),
            ));
        }
        Ok(())
    }
}

/// Nomi dei server: anche cartelle, quindi solo caratteri semplici.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = !name.trim().is_empty()
        && name.len() <= 32
        && name == name.trim()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        && safe_component(name).is_ok();
    if ok {
        Ok(())
    } else {
        Err(ServerError::InvalidName(name.to_owned()))
    }
}

/// Elenco e configurazioni dei server.
#[derive(Debug, Clone)]
pub struct ServerManager {
    paths: Paths,
}

impl ServerManager {
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Cartella dei file del server.
    pub fn dir(&self, name: &str) -> Result<PathBuf> {
        validate_name(name)?;
        Ok(self.paths.server_dir(name)?)
    }

    fn config_file(&self, name: &str) -> Result<PathBuf> {
        validate_name(name)?;
        Ok(self.paths.servers_dir().join(format!("{name}.server.toml")))
    }

    pub fn exists(&self, name: &str) -> bool {
        self.config_file(name).is_ok_and(|p| p.is_file())
    }

    pub fn load(&self, name: &str) -> Result<ServerConfig> {
        let path = self.config_file(name)?;
        let text =
            std::fs::read_to_string(&path).map_err(|_| ServerError::NotFound(name.to_owned()))?;
        let config: ServerConfig =
            toml::from_str(&text).map_err(|e| ServerError::Toml(e.to_string()))?;
        if config.name != name {
            return Err(ServerError::Toml("name does not match the file".into()));
        }
        Ok(config)
    }

    pub fn save(&self, config: &ServerConfig) -> Result<()> {
        config.validate()?;
        let text = toml::to_string_pretty(config).map_err(|e| ServerError::Toml(e.to_string()))?;
        fs::write_atomic(&self.config_file(&config.name)?, text.as_bytes())?;
        Ok(())
    }

    /// Tutti i server, in ordine di creazione. Quelli illeggibili si saltano.
    pub fn list(&self) -> Vec<ServerConfig> {
        let Ok(entries) = std::fs::read_dir(self.paths.servers_dir()) else {
            return Vec::new();
        };
        let mut list: Vec<ServerConfig> = entries
            .flatten()
            .filter_map(|e| {
                let file = e.file_name().into_string().ok()?;
                let name = file.strip_suffix(".server.toml")?;
                self.load(name).ok()
            })
            .collect();
        list.sort_by_key(|c| c.created);
        list
    }

    /// Immagine del server (`server-icon.png`), se c'è.
    pub fn icon(&self, name: &str) -> Option<Vec<u8>> {
        let path = self.dir(name).ok()?.join(ICON);
        // Un'icona vera pesa pochi KB: un file enorme non si legge.
        if std::fs::metadata(&path).ok()?.len() > MAX_ICON_BYTES {
            return None;
        }
        std::fs::read(path).ok()
    }

    /// Imposta (`Some`, PNG 64×64 come vuole Minecraft) o toglie l'immagine.
    /// Il gioco la mostra nella lista dei server; vale dal prossimo avvio.
    pub fn set_icon(&self, name: &str, png: Option<&[u8]>) -> Result<()> {
        self.load(name)?;
        let path = self.dir(name)?.join(ICON);
        match png {
            Some(bytes) => {
                check_icon(bytes)?;
                fs::write_atomic(&path, bytes)?;
            }
            None => match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            },
        }
        Ok(())
    }

    /// Elimina configurazione e file del server (va fermato prima).
    pub fn delete(&self, name: &str) -> Result<()> {
        let dir = self.dir(name)?;
        fs::remove_dir_all_if_exists(&dir)?;
        let config = self.config_file(name)?;
        match std::fs::remove_file(&config) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(name: &str) -> ServerConfig {
        ServerConfig {
            name: name.into(),
            kind: ServerKind::Paper,
            version: "1.21.4".into(),
            build: Some(232),
            memory_mb: 4096,
            port: 25565,
            keep_running: true,
            autostart: false,
            created: 1,
            java_component: "java-runtime-delta".into(),
            playit_tunnel: None,
            playit_address: None,
        }
    }

    #[test]
    fn names_are_restricted() {
        assert!(validate_name("Server degli amici").is_ok());
        assert!(validate_name("../x").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name(" spazio").is_err());
        assert!(validate_name("con").is_err(), "reserved on Windows");
        assert!(validate_name(&"x".repeat(33)).is_err());
    }

    #[test]
    fn saves_lists_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let manager = ServerManager::new(Paths::new(dir.path()));
        manager.save(&config("Amici")).unwrap();
        std::fs::create_dir_all(manager.dir("Amici").unwrap()).unwrap();
        assert_eq!(manager.list().len(), 1);
        assert_eq!(manager.load("Amici").unwrap().build, Some(232));
        let mut bad = config("Amici");
        bad.port = 80;
        assert!(manager.save(&bad).is_err());
        manager.delete("Amici").unwrap();
        assert!(manager.list().is_empty());
        assert!(!manager.dir("Amici").unwrap().exists());
    }

    /// Intestazione minima di un PNG `w`×`h` (basta per il controllo).
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        b.extend(w.to_be_bytes());
        b.extend(h.to_be_bytes());
        b.extend([8, 6, 0, 0, 0]);
        b
    }

    #[test]
    fn server_icon_must_be_a_64px_png() {
        let dir = tempfile::tempdir().unwrap();
        let manager = ServerManager::new(Paths::new(dir.path()));
        manager.save(&config("Amici")).unwrap();
        std::fs::create_dir_all(manager.dir("Amici").unwrap()).unwrap();
        assert!(manager.set_icon("Amici", Some(&png(128, 128))).is_err());
        assert!(
            manager
                .set_icon("Amici", Some(b"GIF89a not a png"))
                .is_err()
        );
        assert!(manager.set_icon("Nessuno", Some(&png(64, 64))).is_err());
        manager.set_icon("Amici", Some(&png(64, 64))).unwrap();
        assert_eq!(manager.icon("Amici"), Some(png(64, 64)));
        manager.set_icon("Amici", None).unwrap();
        assert_eq!(manager.icon("Amici"), None);
        manager.set_icon("Amici", None).unwrap();
    }
}
