//! `server.properties`: letto e riscritto conservando ordine, commenti e
//! chiavi sconosciute; le impostazioni comuni hanno una vista tipizzata per
//! l'editor grafico.

use std::path::Path;

use hyper_core::fs;
use serde::{Deserialize, Serialize};

use crate::{Result, ServerError};

pub const FILE: &str = "server.properties";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    /// Commento o riga vuota, riscritti così come sono.
    Other(String),
    Entry {
        key: String,
        value: String,
    },
}

/// Il file come sequenza di righe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Properties {
    lines: Vec<Line>,
}

impl Properties {
    pub fn parse(text: &str) -> Self {
        let lines = text
            .lines()
            .map(|raw| {
                let trimmed = raw.trim_start();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                    return Line::Other(raw.to_owned());
                }
                let (key, value) = split_entry(trimmed);
                Line::Entry {
                    key: unescape(key),
                    value: unescape(value),
                }
            })
            .collect();
        Self { lines }
    }

    pub fn load(dir: &Path) -> Result<Self> {
        match std::fs::read_to_string(dir.join(FILE)) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        fs::write_atomic(&dir.join(FILE), self.to_string().as_bytes())?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|l| match l {
            Line::Entry { key: k, value } if k == key => Some(value.as_str()),
            _ => None,
        })
    }

    /// Imposta un valore: sostituisce la riga esistente o la aggiunge in fondo.
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        for line in &mut self.lines {
            if let Line::Entry { key: k, value: v } = line
                && k == key
            {
                *v = value;
                return;
            }
        }
        self.lines.push(Line::Entry {
            key: key.to_owned(),
            value,
        });
    }

    fn bool(&self, key: &str, default: bool) -> bool {
        self.get(key).map_or(default, |v| v.trim() == "true")
    }

    fn number(&self, key: &str, default: u32) -> u32 {
        self.get(key)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Le impostazioni comuni (con i valori predefiniti del gioco se mancano).
    pub fn common(&self) -> CommonSettings {
        CommonSettings {
            max_players: self.number("max-players", 20),
            online_mode: self.bool("online-mode", true),
            motd: self.get("motd").unwrap_or("A Minecraft Server").to_owned(),
            difficulty: self.get("difficulty").unwrap_or("easy").to_owned(),
            gamemode: self.get("gamemode").unwrap_or("survival").to_owned(),
            pvp: self.bool("pvp", true),
            whitelist: self.bool("white-list", false),
            view_distance: self.number("view-distance", 10),
            simulation_distance: self.number("simulation-distance", 10),
            hardcore: self.bool("hardcore", false),
            allow_flight: self.bool("allow-flight", false),
            spawn_protection: self.number("spawn-protection", 16),
            seed: self.get("level-seed").unwrap_or_default().to_owned(),
            port: self
                .get("server-port")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(25565),
        }
    }

    pub fn set_common(&mut self, s: &CommonSettings) -> Result<()> {
        s.validate()?;
        self.set("max-players", s.max_players.to_string());
        self.set("online-mode", s.online_mode.to_string());
        self.set("motd", s.motd.clone());
        self.set("difficulty", s.difficulty.clone());
        self.set("gamemode", s.gamemode.clone());
        self.set("pvp", s.pvp.to_string());
        self.set("white-list", s.whitelist.to_string());
        self.set("enforce-whitelist", s.whitelist.to_string());
        self.set("view-distance", s.view_distance.to_string());
        self.set("simulation-distance", s.simulation_distance.to_string());
        self.set("hardcore", s.hardcore.to_string());
        self.set("allow-flight", s.allow_flight.to_string());
        self.set("spawn-protection", s.spawn_protection.to_string());
        self.set("level-seed", s.seed.clone());
        self.set("server-port", s.port.to_string());
        self.set("query.port", s.port.to_string());
        Ok(())
    }
}

impl std::fmt::Display for Properties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.lines {
            match line {
                Line::Other(raw) => writeln!(f, "{raw}")?,
                Line::Entry { key, value } => {
                    writeln!(f, "{}={}", escape(key, true), escape(value, false))?
                }
            }
        }
        Ok(())
    }
}

/// Separa chiave e valore: il primo `=` o `:` non preceduto da `\`.
fn split_entry(line: &str) -> (&str, &str) {
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match c {
            '\\' if !escaped => escaped = true,
            '=' | ':' if !escaped => return (line[..i].trim_end(), line[i + 1..].trim_start()),
            _ => escaped = false,
        }
    }
    (line.trim_end(), "")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => out.push_str(&hex),
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Come fa Java: barre, separatori nella chiave e caratteri non ASCII
/// (es. le lettere accentate del MOTD) diventano sequenze di escape.
fn escape(s: &str, key: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '=' | ':' => {
                out.push('\\');
                out.push(c);
            }
            ' ' if key => out.push_str("\\ "),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Le impostazioni dell'editor grafico.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonSettings {
    pub max_players: u32,
    /// `false` = entrano anche gli account offline.
    pub online_mode: bool,
    pub motd: String,
    pub difficulty: String,
    pub gamemode: String,
    pub pvp: bool,
    pub whitelist: bool,
    pub view_distance: u32,
    pub simulation_distance: u32,
    pub hardcore: bool,
    pub allow_flight: bool,
    pub spawn_protection: u32,
    pub seed: String,
    pub port: u16,
}

impl CommonSettings {
    pub fn validate(&self) -> Result<()> {
        let bad = |msg: &str| Err(ServerError::Invalid(msg.into()));
        if !(1..=1000).contains(&self.max_players) {
            return bad("max players must be between 1 and 1000");
        }
        if !["peaceful", "easy", "normal", "hard"].contains(&self.difficulty.as_str()) {
            return bad("unknown difficulty");
        }
        if !["survival", "creative", "adventure", "spectator"].contains(&self.gamemode.as_str()) {
            return bad("unknown game mode");
        }
        if !(2..=32).contains(&self.view_distance) || !(2..=32).contains(&self.simulation_distance)
        {
            return bad("view and simulation distance must be between 2 and 32");
        }
        if self.spawn_protection > 1000 || self.motd.chars().count() > 200 || self.seed.len() > 64 {
            return bad("value too long");
        }
        if !crate::config::PORTS.contains(&self.port) {
            return bad("port must be between 1024 and 65535");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "#Minecraft server properties\n#Mon Nov 18 21:00:00 CET 2026\nallow-flight=false\nmotd=Ciao \\u00E8 bello\nmax-players=20\ncustom-plugin-key=keep me\nlevel-name=world\n";

    #[test]
    fn keeps_comments_and_unknown_keys() {
        let mut props = Properties::parse(SAMPLE);
        assert_eq!(props.get("motd"), Some("Ciao è bello"));
        let mut common = props.common();
        assert_eq!(common.max_players, 20);
        common.max_players = 42;
        common.motd = "Benvenuti: è qui".into();
        props.set_common(&common).unwrap();
        let text = props.to_string();
        assert!(text.starts_with("#Minecraft server properties\n#Mon"));
        assert!(text.contains("custom-plugin-key=keep me"));
        assert!(text.contains("max-players=42"));
        assert!(text.contains("motd=Benvenuti\\: \\u00E8 qui"));
        // Rilettura identica.
        assert_eq!(Properties::parse(&text).common(), common);
    }

    #[test]
    fn rejects_bad_values() {
        let mut common = Properties::default().common();
        common.difficulty = "impossible".into();
        assert!(common.validate().is_err());
        let mut common = Properties::default().common();
        common.port = 80;
        assert!(common.validate().is_err());
    }
}
