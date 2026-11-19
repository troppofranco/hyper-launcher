//! Accesso da internet con playit.gg: un tunnel gratuito porta gli amici
//! fuori casa al server, senza aprire porte sul router.
//!
//! - **Collegamento** (una volta): il launcher genera un codice, l'utente lo
//!   approva nel browser su `playit.gg/claim/<codice>` e playit restituisce la
//!   chiave segreta di un nuovo agent ("self-managed").
//! - **Tunnel**: con quella chiave si crea un tunnel "Minecraft Java" verso
//!   la porta locale del server e se ne legge l'indirizzo pubblico.
//! - **Agent** (`playitd`, il programma ufficiale): tiene la connessione con
//!   playit mentre il server è acceso; lo avvia l'host dei server.
//!
//! La chiave vive solo nel keyring di sistema e non compare mai nei log o
//! nei messaggi d'errore. Le chiamate vanno solo a `https://api.playit.gg`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use hyper_auth::{KeyringStore, Secret, SecretStore};
use hyper_core::{Paths, fs};
use hyper_installer::Installer;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{Result, ServerError};

const API: &str = "https://api.playit.gg";
/// Voce del keyring con la chiave dell'agent.
const KEYRING_KEY: &str = "playit-agent";

/// Voce del keyring (le prove usano `HYPER_PLAYIT_KEYRING` per non toccare
/// quella vera).
fn keyring_key() -> String {
    std::env::var("HYPER_PLAYIT_KEYRING").unwrap_or_else(|_| KEYRING_KEY.to_owned())
}
/// Per quanto si aspetta che l'utente approvi nel browser.
const CLAIM_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Quanto può metterci un agent appena avviato a presentarsi a playit.
const AGENT_STARTUP: Duration = Duration::from_secs(60);
/// Messaggio quando la chiave non vale più (agent eliminato su playit.gg).
const INVALID_KEY: &str = "il collegamento con playit.gg non è più valido (agent eliminato dal sito): ricollega l'account";

/// Agent ufficiale (BSD-2): versione e SHA256 fissati qui, verificati prima
/// dell'uso. Per aggiornarlo si cambiano questi tre valori.
const AGENT_VERSION: &str = "2.0.2";
const AGENT_URL: &str = "https://github.com/playit-cloud/playit-agent/releases/download/v2.0.2/playitd-windows-x86_64-signed.exe";
const AGENT_SHA256: &str = "6e62da9f56a3bcf72e0c62c29e94609c847cc9b110c460f68b20035525985238";

fn err(msg: impl Into<String>) -> ServerError {
    ServerError::Playit(msg.into())
}

// ---------- Chiave ----------

/// Chiave dell'agent salvata, se l'utente ha già collegato playit.gg.
pub fn secret() -> Result<Option<Secret>> {
    KeyringStore::default()
        .get(&keyring_key())
        .map_err(|e| err(format!("keyring: {e}")))
}

fn save_secret(key: &Secret) -> Result<()> {
    KeyringStore::default()
        .set(&keyring_key(), key)
        .map_err(|e| err(format!("keyring: {e}")))
}

/// Scollega playit.gg da questo PC (l'agent resta nell'account playit, da
/// dove si può eliminare).
pub fn forget() -> Result<()> {
    KeyringStore::default()
        .delete(&keyring_key())
        .map_err(|e| err(format!("keyring: {e}")))
}

// ---------- API ----------

/// Risposta di playit: `{"status":"success"|"fail"|"error","data":…}`.
#[derive(Deserialize)]
#[serde(tag = "status", content = "data")]
enum ApiResult<T> {
    #[serde(rename = "success")]
    Success(T),
    #[serde(rename = "fail")]
    Fail(serde_json::Value),
    #[serde(rename = "error")]
    Error(serde_json::Value),
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
}

impl Client {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("hyper-launcher/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .https_only(true)
            .build()
            .map_err(|e| err(e.to_string()))?;
        Ok(Self { http })
    }

    async fn call<T: DeserializeOwned>(
        &self,
        path: &str,
        key: Option<&Secret>,
        body: &impl Serialize,
    ) -> Result<T> {
        let mut req = self.http.post(format!("{API}{path}")).json(body);
        if let Some(key) = key {
            req = req.header(
                reqwest::header::AUTHORIZATION,
                format!("Agent-Key {}", key.expose().trim()),
            );
        }
        // Gli errori di rete non riportano la richiesta (che ha la chiave).
        let res = req
            .send()
            .await
            .map_err(|e| err(format!("playit.gg non raggiungibile: {}", e.without_url())))?;
        let status = res.status();
        let text = res
            .text()
            .await
            .map_err(|e| err(e.without_url().to_string()))?;
        match serde_json::from_str::<ApiResult<T>>(&text) {
            Ok(ApiResult::Success(v)) => Ok(v),
            Ok(ApiResult::Fail(v)) => Err(err(format!("playit.gg: {}", reason(&v)))),
            // Agent eliminato dal sito di playit: la chiave non vale più e
            // con lei i suoi tunnel. Si dimentica, così l'interfaccia propone
            // di ricollegare l'account.
            Ok(ApiResult::Error(v)) if key.is_some() && reason(&v).contains("InvalidAgentKey") => {
                let _ = forget();
                Err(err(INVALID_KEY))
            }
            Ok(ApiResult::Error(v)) => Err(err(format!("playit.gg: {}", reason(&v)))),
            Err(_) => Err(err(format!("risposta inattesa da playit.gg ({status})"))),
        }
    }

    // --- Collegamento ---

    /// Stato dell'approvazione nel browser.
    async fn claim_setup(&self, code: &str) -> Result<ClaimSetup> {
        #[derive(Serialize)]
        struct Req<'a> {
            code: &'a str,
            agent_type: &'static str,
            version: String,
        }
        self.call(
            "/claim/setup",
            None,
            &Req {
                code,
                agent_type: "self-managed",
                version: format!("Hyper Launcher {}", env!("CARGO_PKG_VERSION")),
            },
        )
        .await
    }

    async fn claim_exchange(&self, code: &str) -> Result<Secret> {
        #[derive(Serialize)]
        struct Req<'a> {
            code: &'a str,
        }
        #[derive(Deserialize)]
        struct Res {
            secret_key: String,
        }
        let res: Res = self.call("/claim/exchange", None, &Req { code }).await?;
        Ok(Secret::new(res.secret_key))
    }

    /// Aspetta che l'utente approvi `code` nel browser (al massimo 10
    /// minuti), poi salva la chiave nel keyring. `on_status` riceve lo stato
    /// a ogni cambio, per l'interfaccia.
    pub async fn claim(&self, code: &str, on_status: impl Fn(ClaimSetup)) -> Result<()> {
        let started = std::time::Instant::now();
        let mut last = None;
        loop {
            if started.elapsed() > CLAIM_TIMEOUT {
                return Err(err("tempo scaduto: approvazione non arrivata"));
            }
            let status = self.claim_setup(code).await?;
            if last != Some(status) {
                on_status(status);
                last = Some(status);
            }
            match status {
                ClaimSetup::UserAccepted => break,
                ClaimSetup::UserRejected => return Err(err("collegamento rifiutato nel browser")),
                _ => tokio::time::sleep(Duration::from_secs(1)).await,
            }
        }
        // Dopo l'approvazione la chiave può arrivare con un attimo di ritardo.
        for _ in 0..30 {
            match self.claim_exchange(code).await {
                Ok(key) => return save_secret(&key),
                Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
            }
        }
        Err(err("playit.gg non ha consegnato la chiave"))
    }

    // --- Agent e tunnel ---

    pub async fn rundata(&self, key: &Secret) -> Result<RunData> {
        self.call("/v1/agents/rundata", Some(key), &serde_json::json!({}))
            .await
    }

    /// Crea un tunnel Minecraft Java verso `127.0.0.1:<port>` su questo agent.
    pub async fn create_tunnel(&self, key: &Secret, name: &str, port: u16) -> Result<Uuid> {
        let agent_id = self.rundata(key).await?.agent_id;
        #[derive(Deserialize)]
        struct Res {
            id: Uuid,
        }
        // Finché l'agent appena avviato non si è presentato a playit, la
        // risposta è "AgentVersionTooOld": si riprova per un po'.
        let started = std::time::Instant::now();
        loop {
            let res = self
                .call::<Res>(
                    "/tunnels/create",
                    Some(key),
                    &create_request(agent_id, name, port),
                )
                .await;
            match res {
                Ok(res) => return Ok(res.id),
                Err(ServerError::Playit(msg))
                    if msg.ends_with("AgentVersionTooOld") && started.elapsed() < AGENT_STARTUP =>
                {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Elimina un tunnel creato dal launcher.
    pub async fn delete_tunnel(&self, key: &Secret, id: Uuid) -> Result<()> {
        self.call::<serde_json::Value>(
            "/tunnels/delete",
            Some(key),
            &serde_json::json!({ "tunnel_id": id }),
        )
        .await
        .map(drop)
    }
}

/// Motivo leggibile da una risposta `fail`/`error` (di solito una stringa
/// come `"RequiresVerifiedAccount"`).
fn reason(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string().chars().take(200).collect(),
    }
}

/// Corpo di `/tunnels/create`: tunnel Minecraft Java (TCP) verso la porta
/// locale del server su questo agent.
fn create_request(agent_id: Uuid, name: &str, port: u16) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "tunnel_type": "minecraft-java",
        "port_type": "tcp",
        "port_count": 1,
        "origin": {
            "type": "agent",
            "data": { "agent_id": agent_id, "local_ip": "127.0.0.1", "local_port": port },
        },
        "enabled": true,
        "alloc": null,
        "firewall_id": null,
        "proxy_protocol": null,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ClaimSetup {
    WaitingForUserVisit,
    WaitingForUser,
    UserAccepted,
    UserRejected,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunData {
    pub agent_id: Uuid,
    #[serde(default)]
    pub tunnels: Vec<Tunnel>,
    #[serde(default)]
    pub pending: Vec<PendingTunnel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tunnel {
    pub id: Uuid,
    pub name: String,
    /// Indirizzo pubblico (es. `amici.gl.joinmc.link`).
    pub display_address: String,
    #[serde(default)]
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PendingTunnel {
    pub id: Uuid,
    pub status_msg: String,
}

/// Codice di collegamento: 5 byte casuali in esadecimale, come l'agent
/// ufficiale.
pub fn claim_code() -> String {
    hex::encode(&Uuid::new_v4().as_bytes()[..5])
}

pub fn claim_url(code: &str) -> String {
    format!("https://playit.gg/claim/{code}")
}

// ---------- Agent ----------

/// Percorso dell'agent `playitd` (scaricato e verificato da [`ensure_agent`]).
pub fn agent_path(paths: &Paths) -> PathBuf {
    paths
        .root()
        .join("tools")
        .join("playit")
        .join(AGENT_VERSION)
        .join("playitd.exe")
}

fn agent_ok(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|b| sha256_hex(&b) == AGENT_SHA256)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Scarica l'agent ufficiale se manca (o non corrisponde all'hash fissato).
pub async fn ensure_agent(installer: &Installer, paths: &Paths) -> Result<PathBuf> {
    let path = agent_path(paths);
    if agent_ok(&path) {
        return Ok(path);
    }
    let bytes = installer.downloader().get_bytes(AGENT_URL).await?;
    crate::install::check_sha256(&bytes, AGENT_SHA256, "playitd.exe")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    fs::write_atomic(&path, &bytes)?;
    Ok(path)
}

/// Avvia l'agent con la chiave del keyring, senza finestra né output su
/// file: la chiave passa solo sulla riga di comando (`--secret`, il modo
/// previsto da playitd), mai su disco.
pub fn spawn_agent(paths: &Paths) -> Result<tokio::process::Child> {
    let exe = agent_ready(paths).ok_or_else(|| err("agent playit non scaricato"))?;
    let key = secret()?.ok_or_else(|| err("playit.gg non collegato"))?;
    let mut command = tokio::process::Command::new(exe);
    command
        .arg("--secret")
        .arg(key.expose().trim())
        // Canale di controllo privato: senza questo playitd usa quello del
        // servizio di sistema (`playitd-system`), aperto a tutti gli utenti
        // Windows del PC, e si scontra con il playit ufficiale se installato.
        .arg("--socket-path")
        .arg(format!(
            r"\\.\pipe\hyper-playitd-{}",
            Uuid::new_v4().simple()
        ))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    Ok(command.spawn()?)
}

/// `true` se c'è già un agent nostro in esecuzione (quello dell'host).
fn agent_running(paths: &Paths) -> bool {
    let path = agent_path(paths);
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::Always),
    );
    system
        .processes()
        .values()
        .any(|p| p.exe().is_some_and(|e| e == path))
}

/// Fasi di [`enable`], per l'avanzamento nell'interfaccia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EnableStage {
    /// Download (verificato) dell'agent ufficiale.
    Agent,
    /// Creazione del tunnel su playit.
    Tunnel,
    /// Attesa dell'indirizzo pubblico.
    Address,
}

/// Rende un server raggiungibile da internet: scarica l'agent se manca, crea
/// il tunnel Minecraft Java verso la sua porta e salva tunnel e indirizzo
/// nella configurazione. Da lì l'host tiene acceso l'agent mentre il server
/// gira. Restituisce l'indirizzo pubblico.
pub async fn enable(
    installer: &Installer,
    paths: &Paths,
    manager: &crate::ServerManager,
    name: &str,
    stage: impl Fn(EnableStage),
) -> Result<String> {
    let mut config = manager.load(name)?;
    if let Some(address) = config
        .playit_address
        .clone()
        .filter(|_| config.playit_tunnel.is_some())
    {
        return Ok(address);
    }
    let key = secret()?.ok_or_else(|| err("playit.gg non collegato"))?;
    let client = Client::new()?;

    stage(EnableStage::Agent);
    ensure_agent(installer, paths).await?;
    // Un agent appena collegato deve presentarsi a playit prima che si
    // possano creare tunnel: se l'host non ne ha già uno acceso, se ne avvia
    // uno temporaneo (si chiude da solo alla fine).
    let _temporary = if agent_running(paths) {
        None
    } else {
        Some(spawn_agent(paths)?)
    };

    stage(EnableStage::Tunnel);
    // Un tunnel già creato (es. la volta prima è mancato solo l'indirizzo)
    // si riusa: niente doppioni nell'account.
    let id = match config.playit_tunnel {
        Some(id) => id,
        None => {
            let id = client
                .create_tunnel(&key, &format!("Hyper Launcher - {name}"), config.port)
                .await?;
            config.playit_tunnel = Some(id);
            manager.save(&config)?;
            id
        }
    };

    stage(EnableStage::Address);
    let started = std::time::Instant::now();
    let address = loop {
        let data = client.rundata(&key).await?;
        if let Some(t) = data.tunnels.iter().find(|t| t.id == id) {
            break t.display_address.clone();
        }
        if started.elapsed() > AGENT_STARTUP {
            return Err(err(
                "playit.gg non ha ancora assegnato un indirizzo: riprova tra poco",
            ));
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    };
    config.playit_address = Some(address.clone());
    manager.save(&config)?;
    Ok(address)
}

/// Toglie l'accesso da internet: elimina il tunnel su playit e lo dimentica.
pub async fn disable(manager: &crate::ServerManager, name: &str) -> Result<()> {
    let mut config = manager.load(name)?;
    // Senza chiave (tolta perché l'agent è stato eliminato dal sito) il
    // tunnel non esiste più: resta solo da dimenticarlo.
    if let (Some(id), Some(key)) = (config.playit_tunnel, secret()?) {
        match Client::new()?.delete_tunnel(&key, id).await {
            Ok(()) => {}
            // Già eliminato dal sito di playit (il tunnel, o l'agent con
            // tutti i suoi tunnel): va bene lo stesso.
            Err(ServerError::Playit(msg))
                if msg.ends_with("TunnelNotFound") || msg == INVALID_KEY => {}
            // Qualunque altro errore si mostra: il tunnel resterebbe attivo
            // nell'account senza che il launcher lo sappia.
            Err(e) => return Err(e),
        }
    }
    config.playit_tunnel = None;
    config.playit_address = None;
    manager.save(&config)
}

/// Scollega playit.gg da questo PC: prima elimina i tunnel dei server (se
/// uno non si riesce a eliminare ci si ferma, chiave compresa), poi toglie
/// la chiave dal keyring.
pub async fn disconnect(manager: &crate::ServerManager) -> Result<()> {
    for config in manager.list() {
        if config.playit_tunnel.is_some() {
            disable(manager, &config.name).await?;
        }
    }
    forget()
}

/// `true` se l'agent è già scaricato e integro (l'host lo avvia solo così).
pub fn agent_ready(paths: &Paths) -> Option<PathBuf> {
    let path = agent_path(paths);
    agent_ok(&path).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_code_and_url() {
        let code = claim_code();
        assert_eq!(code.len(), 10);
        assert!(code.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(code, claim_code());
        assert_eq!(
            claim_url("abcdef0123"),
            "https://playit.gg/claim/abcdef0123"
        );
    }

    #[test]
    fn tunnel_request_shape() {
        let id = Uuid::nil();
        let req = create_request(id, "Hyper Launcher - Amici", 25565);
        assert_eq!(req["tunnel_type"], "minecraft-java");
        assert_eq!(req["port_type"], "tcp");
        assert_eq!(req["origin"]["type"], "agent");
        assert_eq!(req["origin"]["data"]["local_ip"], "127.0.0.1");
        assert_eq!(req["origin"]["data"]["local_port"], 25565);
    }

    #[test]
    fn parses_api_results() {
        let ok: ApiResult<RunData> = serde_json::from_str(
            r#"{"status":"success","data":{"agent_id":"00000000-0000-0000-0000-000000000000","tunnels":[{"id":"00000000-0000-0000-0000-000000000001","name":"x","display_address":"a.gl.joinmc.link","port_type":"tcp"}],"pending":[]}}"#,
        )
        .unwrap();
        assert!(
            matches!(ok, ApiResult::Success(d) if d.tunnels[0].display_address == "a.gl.joinmc.link")
        );
        let fail: ApiResult<RunData> =
            serde_json::from_str(r#"{"status":"fail","data":"RequiresVerifiedAccount"}"#).unwrap();
        assert!(matches!(fail, ApiResult::Fail(v) if reason(&v) == "RequiresVerifiedAccount"));
        let setup: ClaimSetup = serde_json::from_str(r#""WaitingForUser""#).unwrap();
        assert_eq!(setup, ClaimSetup::WaitingForUser);
    }

    #[test]
    fn pinned_agent_hash_is_checked() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("playitd.exe");
        std::fs::write(&file, b"not the agent").unwrap();
        assert!(!agent_ok(&file));
        assert_eq!(sha256_hex(b"abc").len(), 64);
    }
}
