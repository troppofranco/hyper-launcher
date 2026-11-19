//! Operatori, whitelist e ban dai file JSON del server, e i giocatori
//! online ricavati dalla console.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub uuid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BannedEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PlayerLists {
    pub ops: Vec<PlayerEntry>,
    pub whitelist: Vec<PlayerEntry>,
    pub banned: Vec<BannedEntry>,
}

fn read<T: for<'de> Deserialize<'de>>(dir: &Path, file: &str) -> Vec<T> {
    std::fs::read(dir.join(file))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Liste del server; un file mancante o illeggibile è una lista vuota.
pub fn lists(dir: &Path) -> PlayerLists {
    PlayerLists {
        ops: read(dir, "ops.json"),
        whitelist: read(dir, "whitelist.json"),
        banned: read(dir, "banned-players.json"),
    }
}

/// Entrata o uscita di un giocatore da una riga di console
/// (`[..] Steve joined the game`). `true` = entrato.
pub fn join_or_leave(line: &str) -> Option<(String, bool)> {
    let text = line.rsplit_once("]: ").map_or(line, |(_, t)| t).trim();
    let (name, joined) = text
        .strip_suffix(" joined the game")
        .map(|n| (n, true))
        .or_else(|| text.strip_suffix(" left the game").map(|n| (n, false)))?;
    // Nomi di Minecraft: 3–16 caratteri, lettere, numeri e _.
    let valid = (1..=16).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    valid.then(|| (name.to_owned(), joined))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_joins_and_leaves() {
        assert_eq!(
            join_or_leave("[21:20:41 INFO]: Marco joined the game"),
            Some(("Marco".into(), true))
        );
        assert_eq!(
            join_or_leave("[21:20:41] [Server thread/INFO]: Giulia_2 left the game"),
            Some(("Giulia_2".into(), false))
        );
        assert_eq!(
            join_or_leave("[21:22:10 INFO]: <Marco> I joined the game"),
            None,
            "chat is not a join"
        );
        assert_eq!(join_or_leave("Done (8.4s)!"), None);
    }

    #[test]
    fn reads_lists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ops.json"),
            r#"[{"uuid":"u1","name":"Marco","level":4,"bypassesPlayerLimit":false}]"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("banned-players.json"), "not json").unwrap();
        let lists = lists(dir.path());
        assert_eq!(lists.ops[0].name, "Marco");
        assert!(lists.banned.is_empty());
    }
}
