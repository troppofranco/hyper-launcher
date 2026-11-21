//! Il gioco installato: dove sta, che versione è, dove tiene i dati, se è
//! aperto, come si avvia.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{BedrockError, Result, Scope};

/// Nome dell'eseguibile del gioco (da `MicrosoftGame.Config`).
const EXE: &str = "Minecraft.Windows.exe";
/// Cartella del gioco dentro `XboxGames` (la seconda è il nome alternativo
/// usato da alcune installazioni).
const CONTENT_DIRS: &[&str] = &[
    r"XboxGames\Minecraft for Windows\Content",
    r"XboxGames\7792D9CE-355A-493C-AFBD-768F4A77C3B0\Content",
];

/// Il gioco installato dal Microsoft Store.
#[derive(Debug, Clone)]
pub struct Game {
    pub content: PathBuf,
    /// Versione leggibile, es. "1.21.130".
    pub version: String,
}

impl Game {
    /// Cerca il gioco su tutti i dischi (o in `HYPER_BEDROCK_CONTENT`).
    pub fn detect() -> Option<Self> {
        let candidates: Vec<PathBuf> = match std::env::var_os("HYPER_BEDROCK_CONTENT") {
            Some(dir) => vec![PathBuf::from(dir)],
            None => ('C'..='Z')
                .flat_map(|d| {
                    CONTENT_DIRS
                        .iter()
                        .map(move |c| PathBuf::from(format!("{d}:\\{c}")))
                })
                .collect(),
        };
        candidates.into_iter().find_map(|content| {
            let config = std::fs::read_to_string(content.join("MicrosoftGame.Config")).ok()?;
            let version = parse_version(&config)?;
            Some(Self { content, version })
        })
    }
}

/// Dalla riga `<Identity ... Version="1.21.13001.0" />` a "1.21.130": la
/// terza parte unisce patch e build (patch × 100 + build).
fn parse_version(config: &str) -> Option<String> {
    let identity = &config[config.find("<Identity")?..];
    let identity = &identity[..identity.find('>')?];
    let start = identity.find("Version=\"")? + "Version=\"".len();
    let raw = &identity[start..];
    let raw = &raw[..raw.find('"')?];
    let parts: Vec<u32> = raw
        .split('.')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [major, minor, patch, ..] => {
            let patch = if *patch >= 100 { patch / 100 } else { *patch };
            Some(format!("{major}.{minor}.{patch}"))
        }
        _ => None,
    }
}

/// Le due cartelle `com.mojang` del gioco.
#[derive(Debug, Clone)]
pub struct DataDirs {
    pub shared: PathBuf,
    /// Quella del profilo Xbox; `None` finché il gioco non è stato aperto
    /// almeno una volta con un account.
    pub user: Option<PathBuf>,
}

impl DataDirs {
    /// `%APPDATA%\Minecraft Bedrock` (o `HYPER_BEDROCK_DATA` nei test).
    pub fn locate() -> Option<Self> {
        let root = match std::env::var_os("HYPER_BEDROCK_DATA") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(std::env::var_os("APPDATA")?).join("Minecraft Bedrock"),
        };
        Some(Self::in_root(&root))
    }

    fn in_root(root: &Path) -> Self {
        let users = root.join("Users");
        let mojang = |dir: &Path| dir.join("games").join("com.mojang");
        // Con più profili Xbox si usa quello usato più di recente.
        let user = std::fs::read_dir(&users)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| !e.file_name().eq_ignore_ascii_case("Shared"))
            .map(|e| mojang(&e.path()))
            .filter(|p| p.is_dir())
            .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());
        Self {
            shared: mojang(&users.join("Shared")),
            user,
        }
    }

    /// Percorso di una cartella gestita dentro il gioco.
    pub fn game_folder(&self, scope: Scope, name: &str) -> Option<PathBuf> {
        match scope {
            Scope::Shared => Some(self.shared.join(name)),
            Scope::User => self.user.as_ref().map(|u| u.join(name)),
        }
    }
}

/// Aiutante di avvio del Microsoft Store: parte prima del gioco.
const HELPER: &str = "gamelaunchhelper.exe";

fn process_running(name: &str) -> bool {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    system
        .processes()
        .values()
        .any(|p| p.name().eq_ignore_ascii_case(name))
}

/// `true` se Minecraft Bedrock è aperto (anche se avviato fuori dal launcher).
pub fn running() -> bool {
    process_running(EXE)
}

/// Aspetta che non ci sia un aiutante di avvio rimasto da un avvio
/// precedente (al massimo `timeout`): finché c'è, Windows ignora le nuove
/// richieste di aprire il gioco. Succede per un paio di minuti se il gioco
/// viene riaperto subito dopo essere stato chiuso.
pub async fn wait_for_helper(timeout: Duration) {
    let started = Instant::now();
    while process_running(HELPER) && started.elapsed() < timeout {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Indirizzo che apre il gioco (lo stesso dei link "unisciti" di
/// Minecraft): va aperto con `ShellExecute`, come un link. Da `cmd /C start`
/// in un processo senza console l'aiutante di avvio del Microsoft Store
/// (`gamelaunchhelper.exe`) resta bloccato e il gioco non parte.
pub const LAUNCH_URI: &str = "minecraft:";

/// Aspetta che il gioco compaia tra i processi (al massimo `timeout`).
pub async fn wait_for_start(timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while !running() {
        if started.elapsed() > timeout {
            return Err(BedrockError::DidNotStart);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

/// Aspetta che il gioco si chiuda.
pub async fn wait_for_exit() {
    while running() {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_game_version() {
        let config = r#"<Game><Identity Name="Microsoft.MinecraftUWP"
            Publisher="CN=Microsoft" Version="1.21.13001.0" /></Game>"#;
        assert_eq!(parse_version(config).as_deref(), Some("1.21.130"));
        let config = r#"<Identity Name="x" Version="1.26.6501.0" />"#;
        assert_eq!(parse_version(config).as_deref(), Some("1.26.65"));
        assert_eq!(parse_version("<Game/>"), None);
    }

    #[test]
    fn finds_the_most_recent_profile() {
        let root = tempfile::tempdir().unwrap();
        let shared = root.path().join("Users/Shared/games/com.mojang");
        std::fs::create_dir_all(&shared).unwrap();
        assert!(DataDirs::in_root(root.path()).user.is_none());
        let user = root.path().join("Users/123456/games/com.mojang");
        std::fs::create_dir_all(&user).unwrap();
        let dirs = DataDirs::in_root(root.path());
        assert_eq!(dirs.user.as_deref(), Some(user.as_path()));
        assert_eq!(dirs.shared, shared);
    }
}
