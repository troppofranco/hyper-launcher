//! Personalizzazione e Home: skin, immagini delle istanze, server salvati.

use std::collections::BTreeMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hyper_auth::AccountStore;
use hyper_instances::servers::{self, SavedServer};
use hyper_instances::skins::{self, SkinLibrary};
use serde::Serialize;
use tauri::State;

use crate::state::{AppState, CmdResult, OrCmd};
use crate::views::InstanceView;

/// Immagine come `data:` URL, l'unico modo di mostrarla con la CSP dell'app.
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

/// Base64 inviato dall'interfaccia (con o senza prefisso `data:`).
fn decode(data: &str) -> CmdResult<Vec<u8>> {
    let raw = data.split_once(";base64,").map_or(data, |(_, b)| b);
    STANDARD
        .decode(raw.trim())
        .map_err(|_| "Immagine non valida.".to_owned())
}

// ---------- Immagine delle istanze ----------

/// Imposta (PNG in base64, già ridimensionato dall'interfaccia) o toglie
/// l'immagine di un'istanza.
#[tauri::command]
pub fn set_instance_icon(
    state: State<'_, AppState>,
    name: String,
    png: Option<String>,
) -> CmdResult<InstanceView> {
    let instance = state.instances().load(&name).cmd()?;
    let bytes = png.as_deref().map(decode).transpose()?;
    instance.set_icon(bytes.as_deref()).cmd()?;
    Ok(InstanceView::new(&instance))
}

// ---------- Skin ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinView {
    pub id: String,
    pub name: String,
    pub slim: bool,
    pub image: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinsView {
    pub skins: Vec<SkinView>,
    /// Skin scelta per l'account attivo.
    pub selected: Option<String>,
    /// Account attivo offline: la skin si vede in gioco tramite resource pack.
    pub offline: bool,
}

/// UUID e tipo dell'account attivo.
fn active_account(state: &AppState) -> CmdResult<Option<(String, bool)>> {
    let store = AccountStore::load(&state.paths.accounts_file()).cmd()?;
    Ok(store
        .select(None)
        .ok()
        .map(|a| (a.uuid().to_string(), a.kind() == "offline")))
}

#[tauri::command]
pub fn list_skins(state: State<'_, AppState>) -> CmdResult<SkinsView> {
    let library = SkinLibrary::new(&state.paths);
    let account = active_account(&state)?;
    let skins = library
        .list()
        .into_iter()
        .filter_map(|s| {
            let png = library.png(&s.id).ok()?;
            Some(SkinView {
                image: data_url("image/png", &png),
                id: s.id,
                name: s.name,
                slim: s.slim,
            })
        })
        .collect();
    Ok(SkinsView {
        skins,
        selected: account
            .as_ref()
            .and_then(|(uuid, _)| library.selected(uuid))
            .map(|s| s.id),
        offline: account.is_some_and(|(_, offline)| offline),
    })
}

/// Skin predefinite di Minecraft, lette dal gioco già scaricato.
#[tauri::command]
pub async fn default_skins(state: State<'_, AppState>) -> CmdResult<Vec<SkinView>> {
    let paths = state.paths.clone();
    let skins = tauri::async_runtime::spawn_blocking(move || skins::default_skins(&paths))
        .await
        .cmd()?;
    Ok(skins
        .into_iter()
        .map(|(name, slim, png)| SkinView {
            image: data_url("image/png", &png),
            id: format!("default:{name}"),
            name: capitalize(&name),
            slim,
        })
        .collect())
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Aggiunge una skin (PNG 64×64 in base64) e la sceglie per l'account attivo.
#[tauri::command]
pub fn add_skin(
    state: State<'_, AppState>,
    name: String,
    png: String,
    slim: bool,
) -> CmdResult<SkinsView> {
    let library = SkinLibrary::new(&state.paths);
    let skin = library.add(&name, &decode(&png)?, slim).cmd()?;
    if let Some((uuid, _)) = active_account(&state)? {
        library.select(&uuid, Some(&skin.id)).cmd()?;
    }
    list_skins(state)
}

#[tauri::command]
pub fn remove_skin(state: State<'_, AppState>, id: String) -> CmdResult<SkinsView> {
    SkinLibrary::new(&state.paths).remove(&id).cmd()?;
    list_skins(state)
}

/// Sceglie la skin dell'account attivo (`None` = quella predefinita).
#[tauri::command]
pub fn select_skin(state: State<'_, AppState>, id: Option<String>) -> CmdResult<SkinsView> {
    let (uuid, _) =
        active_account(&state)?.ok_or_else(|| "Aggiungi prima un account.".to_owned())?;
    SkinLibrary::new(&state.paths)
        .select(&uuid, id.as_deref())
        .cmd()?;
    list_skins(state)
}

// ---------- Server ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    pub name: String,
    pub address: String,
    pub icon: Option<String>,
    /// Istanze che hanno questo server nell'elenco multigiocatore.
    pub instances: Vec<String>,
}

/// Server salvati in tutte le istanze, senza doppioni (per indirizzo).
#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>) -> CmdResult<Vec<ServerView>> {
    let manager = state.instances();
    tauri::async_runtime::spawn_blocking(move || {
        let mut by_address: BTreeMap<String, ServerView> = BTreeMap::new();
        for instance in manager.list().cmd()? {
            for SavedServer {
                name,
                address,
                icon,
            } in instance.servers()
            {
                // Stesso server scritto in modi diversi (porta predefinita,
                // punto finale, maiuscole): una riga sola.
                let key = address
                    .to_lowercase()
                    .trim_end_matches(":25565")
                    .trim_end_matches('.')
                    .to_owned();
                let entry = by_address.entry(key).or_insert_with(|| ServerView {
                    name,
                    address,
                    // Il gioco salva l'icona come PNG in base64.
                    icon: icon.map(|i| format!("data:image/png;base64,{i}")),
                    instances: Vec::new(),
                });
                entry.instances.push(instance.config.name.clone());
            }
        }
        Ok(by_address.into_values().collect())
    })
    .await
    .cmd()?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatusView {
    pub online: u32,
    pub max: u32,
    pub version: String,
    pub motd: String,
    pub latency_ms: u32,
}

/// Stato di un server (giocatori, versione); errore se non risponde.
#[tauri::command]
pub async fn ping_server(address: String) -> CmdResult<ServerStatusView> {
    let status = servers::ping(&address)
        .await
        .map_err(|_| "Server non raggiungibile.".to_owned())?;
    Ok(ServerStatusView {
        online: status.online,
        max: status.max,
        version: status.version,
        motd: status.motd,
        latency_ms: status.latency_ms,
    })
}
