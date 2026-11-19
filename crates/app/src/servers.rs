//! Comandi dei server ospitati: la logica sta in `hyper-server`. I server
//! accesi li possiede l'host in background (questo stesso eseguibile con
//! `--server-host`), così restano attivi anche a launcher chiuso.

use std::net::{IpAddr, UdpSocket};

use hyper_server::install::{self, CreateStage, NewServer};
use hyper_server::players::{BannedEntry, PlayerEntry};
use hyper_server::properties::{CommonSettings, Properties};
use hyper_server::protocol::{RunningStatus, StatsPoint};
use hyper_server::{
    HostClient, ServerConfig, ServerKind, ServerManager, autostart, files, plugins,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::state::{AppState, CmdResult, OrCmd};

/// Argomento con cui l'eseguibile dell'app fa da host dei server.
pub const HOST_ARG: &str = "--server-host";

fn manager(state: &AppState) -> ServerManager {
    ServerManager::new(state.paths.clone())
}

fn client(state: &AppState) -> HostClient {
    HostClient::new(&state.paths)
}

/// Allinea "Avvia con il computer" ai server (voce Run di Windows).
fn sync_autostart(state: &AppState) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let command = format!("\"{}\" {HOST_ARG} --autostart", exe.display());
    if let Err(e) = autostart::sync(&manager(state), &command) {
        tracing::warn!(error = %e, "cannot update the startup entry");
    }
}

/// Indirizzo del PC nella rete di casa. Si "connette" un socket UDP verso
/// un indirizzo pubblico solo per leggere l'interfaccia scelta: nessun
/// pacchetto viene inviato.
fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfoView {
    pub name: String,
    pub kind: ServerKind,
    pub version: String,
    pub build: Option<u32>,
    pub memory_mb: u32,
    pub port: u16,
    pub keep_running: bool,
    pub autostart: bool,
    pub created: u64,
    pub running: bool,
    pub started_at: Option<u64>,
    pub players: u32,
    pub max_players: u32,
    pub address: String,
    /// Immagine del server (`server-icon.png`) come `data:` URL.
    pub icon: Option<String>,
}

/// Vista dei server: una sola richiesta all'host e un solo indirizzo di rete
/// per tutto l'elenco (la pagina lo rilegge ogni 3 secondi).
async fn infos(state: &AppState, configs: Vec<ServerConfig>) -> Vec<ServerInfoView> {
    let status = client(state).status().await;
    let ip = lan_ip().map_or_else(|| "localhost".to_owned(), |ip| ip.to_string());
    let manager = manager(state);
    configs
        .into_iter()
        .map(|config| {
            let running = status.iter().find(|r| r.name == config.name);
            view(&manager, config, running, &ip)
        })
        .collect()
}

async fn info(state: &AppState, config: ServerConfig) -> CmdResult<ServerInfoView> {
    infos(state, vec![config])
        .await
        .pop()
        .ok_or_else(|| "server not found".to_owned())
}

fn view(
    manager: &ServerManager,
    config: ServerConfig,
    running: Option<&RunningStatus>,
    ip: &str,
) -> ServerInfoView {
    let max_players = match running {
        Some(r) => r.max_players,
        None => manager
            .dir(&config.name)
            .ok()
            .and_then(|d| Properties::load(&d).ok())
            .map_or(20, |p| p.common().max_players),
    };
    ServerInfoView {
        icon: manager
            .icon(&config.name)
            .map(|png| crate::custom::data_url("image/png", &png)),
        address: format!("{ip}:{}", config.port),
        running: running.is_some(),
        started_at: running.map(|r| r.started),
        players: running.map_or(0, |r| r.players),
        max_players,
        name: config.name,
        kind: config.kind,
        version: config.version,
        build: config.build,
        memory_mb: config.memory_mb,
        port: config.port,
        keep_running: config.keep_running,
        autostart: config.autostart,
        created: config.created,
    }
}

#[tauri::command]
pub async fn server_list(state: State<'_, AppState>) -> CmdResult<Vec<ServerInfoView>> {
    Ok(infos(&state, manager(&state).list()).await)
}

#[tauri::command]
pub async fn server_versions(
    state: State<'_, AppState>,
    kind: ServerKind,
) -> CmdResult<Vec<String>> {
    install::versions(&state.installer(), kind).await.cmd()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewServerInput {
    pub name: String,
    pub kind: ServerKind,
    pub version: String,
    pub memory_mb: u32,
    pub port: u16,
    pub keep_running: bool,
    pub autostart: bool,
    pub eula: bool,
}

#[derive(Clone, Serialize)]
struct ServerProgress<'a> {
    name: &'a str,
    stage: &'static str,
    done: usize,
    total: usize,
}

#[tauri::command]
pub async fn server_create(
    app: AppHandle,
    state: State<'_, AppState>,
    input: NewServerInput,
) -> CmdResult<ServerInfoView> {
    let name = input.name.trim().to_owned();
    let request = NewServer {
        name: name.clone(),
        kind: input.kind,
        version: input.version,
        memory_mb: input.memory_mb,
        port: input.port,
        keep_running: input.keep_running,
        autostart: input.autostart,
        eula: input.eula,
    };
    let report = |stage: CreateStage, done: usize, total: usize| {
        let _ = app.emit(
            "server-progress",
            ServerProgress {
                name: &name,
                stage: stage.as_str(),
                done,
                total,
            },
        );
    };
    let config = install::create(
        &state.installer(),
        &manager(&state),
        &request,
        Some(&report),
    )
    .await
    .cmd()?;
    sync_autostart(&state);
    info(&state, config).await
}

async fn is_running(state: &AppState, name: &str) -> bool {
    client(state).status().await.iter().any(|r| r.name == name)
}

#[tauri::command]
pub async fn server_delete(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    if is_running(&state, &name).await {
        return Err("Ferma il server prima di eliminarlo.".into());
    }
    manager(&state).delete(&name).cmd()?;
    sync_autostart(&state);
    Ok(())
}

#[tauri::command]
pub async fn server_start(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    manager(&state).load(&name).cmd()?;
    let client = client(&state);
    client
        .ensure(|| hyper_server::client::spawn_host(&[HOST_ARG]))
        .await
        .cmd()?;
    client.start(&name).await.cmd()
}

#[tauri::command]
pub async fn server_stop(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    client(&state).stop(&name).await.cmd()
}

/// Ferma il server, aspetta che si chiuda (al massimo 40 s) e lo riavvia.
#[tauri::command]
pub async fn server_restart(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    let client = client(&state);
    if is_running(&state, &name).await {
        client.stop(&name).await.cmd()?;
        for _ in 0..80 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if !is_running(&state, &name).await {
                break;
            }
        }
    }
    client
        .ensure(|| hyper_server::client::spawn_host(&[HOST_ARG]))
        .await
        .cmd()?;
    client.start(&name).await.cmd()
}

#[tauri::command]
pub async fn server_command(
    state: State<'_, AppState>,
    name: String,
    line: String,
) -> CmdResult<()> {
    client(&state).command(&name, &line).await.cmd()
}

#[derive(Serialize)]
pub struct ConsoleView {
    pub lines: Vec<String>,
    pub next: u64,
}

/// Righe di console dalla `since`; vuoto se l'host non ha questo server.
#[tauri::command]
pub async fn server_console(
    state: State<'_, AppState>,
    name: String,
    since: u64,
) -> CmdResult<ConsoleView> {
    Ok(match client(&state).console(&name, since).await {
        Ok(chunk) => ConsoleView {
            lines: chunk.lines,
            next: chunk.next,
        },
        Err(_) => ConsoleView {
            lines: Vec::new(),
            next: since,
        },
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub points: Vec<StatsPoint>,
    pub mem_max_mb: u32,
    pub online: Vec<String>,
}

#[tauri::command]
pub async fn server_stats(state: State<'_, AppState>, name: String) -> CmdResult<StatsView> {
    let config = manager(&state).load(&name).cmd()?;
    let stats = client(&state).stats(&name).await.unwrap_or_default();
    Ok(StatsView {
        points: stats.points,
        mem_max_mb: config.memory_mb,
        online: stats.online,
    })
}

/// Imposta (PNG 64×64 in base64, già ridimensionato dall'interfaccia) o
/// toglie l'immagine del server: logo nel launcher e icona nel gioco.
#[tauri::command]
pub async fn server_set_icon(
    state: State<'_, AppState>,
    name: String,
    png: Option<String>,
) -> CmdResult<ServerInfoView> {
    let manager = manager(&state);
    let bytes = png.as_deref().map(crate::custom::decode).transpose()?;
    manager.set_icon(&name, bytes.as_deref()).cmd()?;
    info(&state, manager.load(&name).cmd()?).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerUpdate {
    pub memory_mb: u32,
    pub keep_running: bool,
    pub autostart: bool,
}

#[tauri::command]
pub async fn server_update(
    state: State<'_, AppState>,
    name: String,
    update: ServerUpdate,
) -> CmdResult<ServerInfoView> {
    let manager = manager(&state);
    let mut config = manager.load(&name).cmd()?;
    config.memory_mb = update.memory_mb;
    config.keep_running = update.keep_running;
    config.autostart = update.keep_running && update.autostart;
    manager.save(&config).cmd()?;
    sync_autostart(&state);
    info(&state, config).await
}

#[derive(Serialize)]
pub struct PlayersView {
    pub online: Vec<String>,
    pub ops: Vec<PlayerEntry>,
    pub whitelist: Vec<PlayerEntry>,
    pub banned: Vec<BannedEntry>,
}

#[tauri::command]
pub async fn server_players(state: State<'_, AppState>, name: String) -> CmdResult<PlayersView> {
    let dir = manager(&state).dir(&name).cmd()?;
    let lists = hyper_server::players::lists(&dir);
    let online = client(&state)
        .stats(&name)
        .await
        .map(|s| s.online)
        .unwrap_or_default();
    Ok(PlayersView {
        online,
        ops: lists.ops,
        whitelist: lists.whitelist,
        banned: lists.banned,
    })
}

#[tauri::command]
pub fn server_properties(state: State<'_, AppState>, name: String) -> CmdResult<CommonSettings> {
    let dir = manager(&state).dir(&name).cmd()?;
    Ok(Properties::load(&dir).cmd()?.common())
}

/// Salva le impostazioni comuni in `server.properties` (conservando il
/// resto del file) e allinea la porta nella configurazione del launcher.
#[tauri::command]
pub fn server_save_properties(
    state: State<'_, AppState>,
    name: String,
    properties: CommonSettings,
) -> CmdResult<()> {
    let manager = manager(&state);
    let mut config = manager.load(&name).cmd()?;
    let dir = manager.dir(&name).cmd()?;
    let mut props = Properties::load(&dir).cmd()?;
    props.set_common(&properties).cmd()?;
    props.save(&dir).cmd()?;
    if config.port != properties.port {
        config.port = properties.port;
        manager.save(&config).cmd()?;
    }
    Ok(())
}

#[tauri::command]
pub async fn server_files(
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> CmdResult<Vec<files::ServerFile>> {
    let dir = manager(&state).dir(&name).cmd()?;
    tauri::async_runtime::spawn_blocking(move || files::list(&dir, &path))
        .await
        .cmd()?
        .cmd()
}

#[tauri::command]
pub async fn server_read_file(
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> CmdResult<String> {
    let dir = manager(&state).dir(&name).cmd()?;
    tauri::async_runtime::spawn_blocking(move || files::read_text(&dir, &path))
        .await
        .cmd()?
        .cmd()
}

#[tauri::command]
pub async fn server_write_file(
    state: State<'_, AppState>,
    name: String,
    path: String,
    content: String,
) -> CmdResult<()> {
    let dir = manager(&state).dir(&name).cmd()?;
    tauri::async_runtime::spawn_blocking(move || files::write_text(&dir, &path, &content))
        .await
        .cmd()?
        .cmd()
}

#[tauri::command]
pub async fn server_delete_file(
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> CmdResult<()> {
    let dir = manager(&state).dir(&name).cmd()?;
    tauri::async_runtime::spawn_blocking(move || files::delete(&dir, &path))
        .await
        .cmd()?
        .cmd()
}

/// Cartella e versione di Minecraft di un server Paper (i plugin non
/// esistono su Vanilla).
fn paper_server(state: &AppState, name: &str) -> CmdResult<(std::path::PathBuf, String)> {
    let manager = manager(state);
    let config = manager.load(name).cmd()?;
    if config.kind != ServerKind::Paper {
        return Err("I plugin funzionano solo sui server Paper.".into());
    }
    Ok((manager.dir(name).cmd()?, config.version))
}

#[tauri::command]
pub async fn server_plugins(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<Vec<plugins::InstalledPlugin>> {
    let (dir, _) = paper_server(&state, &name)?;
    plugins::installed(&state.modrinth, &dir).await.cmd()
}

/// Installa un plugin da Modrinth (con le dipendenze obbligatorie);
/// restituisce i file scaricati. Vale dal prossimo avvio del server.
#[tauri::command]
pub async fn server_plugin_install(
    state: State<'_, AppState>,
    name: String,
    project: String,
) -> CmdResult<Vec<String>> {
    let (dir, version) = paper_server(&state, &name)?;
    let installer = state.installer();
    plugins::install(
        &state.modrinth,
        installer.downloader(),
        &dir,
        &project,
        &version,
    )
    .await
    .cmd()
}

/// A server acceso Windows tiene aperti i jar: si possono toccare solo da spento.
async fn plugins_unlocked(state: &AppState, name: &str) -> CmdResult<()> {
    if is_running(state, name).await {
        return Err("Ferma il server per attivare, disattivare o eliminare i plugin.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn server_plugin_set_enabled(
    state: State<'_, AppState>,
    name: String,
    file: String,
    enabled: bool,
) -> CmdResult<String> {
    let (dir, _) = paper_server(&state, &name)?;
    plugins_unlocked(&state, &name).await?;
    plugins::set_enabled(&dir, &file, enabled).cmd()
}

#[tauri::command]
pub async fn server_plugin_delete(
    state: State<'_, AppState>,
    name: String,
    file: String,
) -> CmdResult<()> {
    let (dir, _) = paper_server(&state, &name)?;
    plugins_unlocked(&state, &name).await?;
    plugins::remove(&dir, &file).cmd()
}

/// Alla chiusura del launcher: ferma i server senza "resta acceso".
pub fn on_exit(state: &AppState) {
    let client = client(state);
    tauri::async_runtime::block_on(async move {
        let _ =
            tokio::time::timeout(std::time::Duration::from_secs(3), client.stop_unpinned()).await;
    });
}

/// Modalità host: l'eseguibile dell'app avviato con `--server-host` non apre
/// finestre e gestisce i server finché ce ne sono di accesi.
pub fn run_host(autostart: bool) -> anyhow::Result<()> {
    let paths = hyper_core::Paths::from_env()?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(hyper_server::host::run(paths, autostart))?;
    Ok(())
}
