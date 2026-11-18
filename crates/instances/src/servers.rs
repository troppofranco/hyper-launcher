//! Server multigiocatore: quelli salvati nelle istanze (`servers.dat`, il
//! file della schermata "Multigiocatore") e il loro stato, chiesto come fa
//! il gioco nell'elenco server (Server List Ping).

use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::Instance;

/// Un server salvato in un'istanza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedServer {
    pub name: String,
    /// `host` o `host:porta`, come scritto nel gioco.
    pub address: String,
    /// Icona PNG in base64, se il gioco l'ha salvata.
    pub icon: Option<String>,
}

#[derive(Deserialize)]
struct ServersDat {
    #[serde(default)]
    servers: Vec<ServerEntry>,
}

#[derive(Deserialize)]
struct ServerEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    ip: String,
    icon: Option<String>,
}

impl Instance {
    /// Server dell'elenco multigiocatore; vuoto se il file manca o non si
    /// legge (è solo informativo).
    pub fn servers(&self) -> Vec<SavedServer> {
        let Ok(bytes) = std::fs::read(self.game_dir().join("servers.dat")) else {
            return Vec::new();
        };
        let Ok(dat) = fastnbt::from_bytes::<ServersDat>(&bytes) else {
            return Vec::new();
        };
        dat.servers
            .into_iter()
            .filter(|s| !s.ip.trim().is_empty())
            .map(|s| SavedServer {
                name: if s.name.trim().is_empty() {
                    s.ip.clone()
                } else {
                    s.name
                },
                address: s.ip.trim().to_owned(),
                icon: s.icon.filter(|i| !i.is_empty()),
            })
            .collect()
    }
}

/// Stato di un server online.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStatus {
    pub online: u32,
    pub max: u32,
    /// Versione dichiarata (es. "Paper 1.21.4" o "1.8.x-1.21.x").
    pub version: String,
    /// Messaggio del giorno, senza codici colore.
    pub motd: String,
    pub latency_ms: u32,
}

#[derive(Deserialize)]
struct StatusJson {
    #[serde(default)]
    players: Option<Players>,
    #[serde(default)]
    version: Option<VersionInfo>,
    #[serde(default)]
    description: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Players {
    /// Alcuni proxy mandano campi mancanti o negativi: si legge quel che c'è.
    #[serde(default)]
    online: i64,
    #[serde(default)]
    max: i64,
}

#[derive(Deserialize)]
struct VersionInfo {
    name: String,
}

const TIMEOUT: Duration = Duration::from_secs(4);
/// Limite alla risposta: un JSON di stato normale sta in pochi KB (icona
/// compresa). Un server che dichiara di più viene ignorato.
const MAX_RESPONSE: usize = 256 * 1024;

/// Separa `host[:porta]` (porta predefinita 25565). Gli indirizzi IPv6 vanno
/// tra parentesi quadre, come nel gioco.
pub fn split_address(address: &str) -> Option<(String, u16)> {
    let address = address.trim();
    if address.is_empty() || address.len() > 255 {
        return None;
    }
    if let Some(rest) = address.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = match tail.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None if tail.is_empty() => 25565,
            None => return None,
        };
        return Some((host.to_owned(), port));
    }
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Some((host.to_owned(), port.parse().ok()?)),
        // IPv6 senza parentesi (il gioco lo accetta): tutto è l'host.
        Some(_) => Some((address.to_owned(), 25565)),
        None => Some((address.to_owned(), 25565)),
    }
}

/// Chiede lo stato a un server (protocollo "status" di Minecraft Java).
/// Non segue i record SRV: per quei server il risultato è un errore, come
/// "non raggiungibile".
pub async fn ping(address: &str) -> std::io::Result<ServerStatus> {
    let invalid = |msg: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_owned());
    let (host, port) = split_address(address).ok_or_else(|| invalid("invalid address"))?;
    let work = async {
        // La latenza misura solo la connessione, non la risoluzione DNS.
        let addr = tokio::net::lookup_host((host.as_str(), port))
            .await?
            .next()
            .ok_or_else(|| invalid("host not found"))?;
        let started = std::time::Instant::now();
        let mut stream = TcpStream::connect(addr).await?;
        let latency_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

        // Handshake (id 0): protocollo -1 = "qualsiasi", stato successivo 1.
        let mut handshake = Vec::new();
        write_varint(&mut handshake, 0);
        write_varint(&mut handshake, -1);
        write_varint(&mut handshake, host.len() as i32);
        handshake.extend_from_slice(host.as_bytes());
        handshake.extend_from_slice(&port.to_be_bytes());
        write_varint(&mut handshake, 1);
        let mut packet = Vec::new();
        write_varint(&mut packet, handshake.len() as i32);
        packet.extend_from_slice(&handshake);
        // Richiesta di stato: pacchetto vuoto con id 0.
        packet.extend_from_slice(&[1, 0]);
        stream.write_all(&packet).await?;

        let length = read_varint(&mut stream).await?;
        if length <= 0 || length as usize > MAX_RESPONSE {
            return Err(invalid("bad response length"));
        }
        let mut body = vec![0u8; length as usize];
        stream.read_exact(&mut body).await?;
        let mut cursor = &body[..];
        let id = read_varint_slice(&mut cursor).ok_or_else(|| invalid("bad packet"))?;
        let len = read_varint_slice(&mut cursor).ok_or_else(|| invalid("bad packet"))?;
        if id != 0 || len < 0 || len as usize > cursor.len() {
            return Err(invalid("bad status packet"));
        }
        let json: StatusJson =
            serde_json::from_slice(&cursor[..len as usize]).map_err(|e| invalid(&e.to_string()))?;
        Ok(ServerStatus {
            online: json
                .players
                .as_ref()
                .map_or(0, |p| p.online.clamp(0, u32::MAX.into()) as u32),
            max: json
                .players
                .as_ref()
                .map_or(0, |p| p.max.clamp(0, u32::MAX.into()) as u32),
            version: json
                .version
                .map(|v| strip_colors(&v.name))
                .unwrap_or_default(),
            motd: json.description.as_ref().map(motd_text).unwrap_or_default(),
            latency_ms,
        })
    };
    tokio::time::timeout(TIMEOUT, work)
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"))?
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        if v & !0x7f == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v & 0x7f) as u8 | 0x80);
        v >>= 7;
    }
}

async fn read_varint(stream: &mut TcpStream) -> std::io::Result<i32> {
    let mut value = 0u32;
    for i in 0..5 {
        let byte = stream.read_u8().await?;
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "varint too long",
    ))
}

fn read_varint_slice(input: &mut &[u8]) -> Option<i32> {
    let mut value = 0u32;
    for i in 0..5 {
        let (&byte, rest) = input.split_first()?;
        *input = rest;
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some(value as i32);
        }
    }
    None
}

/// Toglie i codici colore `§x` dai testi del server.
fn strip_colors(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// Testo semplice del MOTD, che può essere una stringa o un componente JSON
/// con `text` ed `extra`.
fn motd_text(value: &serde_json::Value) -> String {
    fn collect(value: &serde_json::Value, out: &mut String) {
        match value {
            serde_json::Value::String(s) => out.push_str(s),
            serde_json::Value::Object(map) => {
                if let Some(text) = map.get("text") {
                    collect(text, out);
                }
                if let Some(serde_json::Value::Array(extra)) = map.get("extra") {
                    extra.iter().for_each(|e| collect(e, out));
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|e| collect(e, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    collect(value, &mut out);
    strip_colors(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use hyper_core::Paths;

    use super::*;
    use crate::InstanceManager;

    #[test]
    fn splits_addresses() {
        assert_eq!(
            split_address("play.example.net"),
            Some(("play.example.net".into(), 25565))
        );
        assert_eq!(
            split_address("1.2.3.4:25570"),
            Some(("1.2.3.4".into(), 25570))
        );
        assert_eq!(split_address("[::1]:25566"), Some(("::1".into(), 25566)));
        assert_eq!(split_address("[::1]"), Some(("::1".into(), 25565)));
        assert_eq!(split_address("host:notaport"), None);
        assert_eq!(
            split_address("2001:db8::1"),
            Some(("2001:db8::1".into(), 25565))
        );
        assert_eq!(split_address(""), None);
    }

    #[test]
    fn varints_round_trip() {
        for v in [0, 1, 127, 128, 25565, -1, i32::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let mut slice = &buf[..];
            assert_eq!(read_varint_slice(&mut slice), Some(v));
            assert!(slice.is_empty());
        }
    }

    #[test]
    fn motd_from_text_components() {
        let json =
            serde_json::json!({"text": "§aBenvenuti ", "extra": [{"text": "su"}, " §lHyper"]});
        assert_eq!(motd_text(&json), "Benvenuti su Hyper");
        assert_eq!(motd_text(&serde_json::json!("§6Ciao  mondo")), "Ciao mondo");
    }

    #[test]
    fn reads_servers_dat() {
        #[derive(serde::Serialize)]
        struct Dat {
            servers: Vec<Entry>,
        }
        #[derive(serde::Serialize)]
        struct Entry {
            name: String,
            ip: String,
        }
        let dir = tempfile::tempdir().unwrap();
        let instance = InstanceManager::new(Paths::new(dir.path()))
            .create("s", "26.3", None)
            .unwrap();
        assert!(instance.servers().is_empty());
        let dat = Dat {
            servers: vec![
                Entry {
                    name: "Amici".into(),
                    ip: "mc.example.org".into(),
                },
                Entry {
                    name: "".into(),
                    ip: "1.2.3.4:25570".into(),
                },
                Entry {
                    name: "vuoto".into(),
                    ip: " ".into(),
                },
            ],
        };
        std::fs::write(
            instance.game_dir().join("servers.dat"),
            fastnbt::to_bytes(&dat).unwrap(),
        )
        .unwrap();
        let servers = instance.servers();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "Amici");
        assert_eq!(
            servers[1].name, "1.2.3.4:25570",
            "missing name falls back to the address"
        );
    }
}
