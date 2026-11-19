//! Protocollo tra launcher (o CLI) e host dei server: una richiesta JSON per
//! connessione, una risposta JSON, su TCP solo locale (127.0.0.1). Ogni
//! richiesta porta il token scritto dall'host nel file `server-host.json`.

use serde::{Deserialize, Serialize};

/// Limite di una riga del protocollo (le risposte di console sono le più grandi).
pub const MAX_LINE: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Ping,
    Status,
    Start {
        name: String,
    },
    Stop {
        name: String,
    },
    Kill {
        name: String,
    },
    Command {
        name: String,
        line: String,
    },
    Console {
        name: String,
        since: u64,
    },
    Stats {
        name: String,
    },
    /// Ferma i server senza "resta acceso" (chiusura del launcher).
    StopUnpinned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub token: String,
    #[serde(flatten)]
    pub op: Op,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Dove trovare l'host (scritto da lui all'avvio).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

/// Un server acceso.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunningStatus {
    pub name: String,
    pub started: u64,
    pub players: u32,
    pub max_players: u32,
    pub pid: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsoleChunk {
    pub lines: Vec<String>,
    pub next: u64,
}

/// Un campione delle statistiche (uno ogni 2 secondi).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatsPoint {
    pub t: u64,
    /// Percentuale di tutta la CPU del PC.
    pub cpu: f32,
    pub mem_mb: u32,
    pub players: u32,
    pub latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub points: Vec<StatsPoint>,
    pub online: Vec<String>,
}
