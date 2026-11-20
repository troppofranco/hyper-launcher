//! Comandi dei server ospitati: la logica sta in `hyper-server`. I server
//! accesi li possiede l'host in background (questo stesso eseguibile con
//! `--server-host`), così restano attivi anche a launcher chiuso.

use std::net::{IpAddr, UdpSocket};

use hyper_server::install::{self, CreateStage, NewServer};
use hyper_server::players::{BannedEntry, PlayerEntry};
use hyper_server::properties::{CommonSettings, Properties};
use hyper_server::protocol::{RunningStatus, StatsPoint};
use hyper_server::{
    HostClient, ServerConfig, ServerKind, ServerManager, autostart, files, playit, plugins,
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
    /// Versione del loader (Fabric, NeoForge, Forge).
    pub loader_version: Option<String>,
    /// Modpack di origine (nome e progetto Modrinth), se c'è.
    pub modpack: Option<hyper_server::config::ModpackSource>,
    /// Automatismi (riavvio giornaliero, backup automatici).
    pub schedule: hyper_server::config::Schedule,
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
    /// Indirizzo pubblico (playit.gg), se l'accesso da internet è attivo.
    pub public_address: Option<String>,
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
        public_address: config
            .playit_address
            .clone()
            .filter(|_| config.playit_tunnel.is_some()),
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
        loader_version: config.loader_version.clone(),
        modpack: config.modpack.clone(),
        schedule: config.schedule.clone(),
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
    /// Versione del loader (Fabric, NeoForge, Forge); assente = l'ultima.
    #[serde(default)]
    pub loader_version: Option<String>,
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
        loader_version: input.loader_version,
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

/// Un `.mrpack` scelto dal PC, con quello che serve per mostrarlo.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedModpack {
    pub path: String,
    pub name: String,
    pub game: String,
    /// "Fabric 0.16.10", oppure `None` per un pack vanilla.
    pub loader: Option<String>,
}

/// Fa scegliere un `.mrpack` e ne legge l'indice; `None` se si annulla.
#[tauri::command]
pub async fn server_pick_modpack(app: AppHandle) -> CmdResult<Option<PickedModpack>> {
    use tauri_plugin_dialog::DialogExt;
    let Some(file) = app
        .dialog()
        .file()
        .set_title("Scegli un modpack Modrinth")
        .add_filter("Modpack Modrinth", &["mrpack"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().cmd()?;
    let index = hyper_content::mrpack::read_index(&path).cmd()?;
    let (game, loader) = hyper_content::mrpack::requirements(&index).cmd()?;
    Ok(Some(PickedModpack {
        path: path.display().to_string(),
        name: index.name,
        game,
        loader: loader.map(|l| format!("{} {}", l.kind.display_name(), l.version)),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackServerInput {
    pub name: String,
    pub memory_mb: u32,
    pub port: u16,
    pub keep_running: bool,
    pub autostart: bool,
    pub eula: bool,
    /// Progetto Modrinth (id o slug), oppure…
    pub project: Option<String>,
    /// …un `.mrpack` scelto con `server_pick_modpack`.
    pub file: Option<String>,
}

/// Crea un server da un modpack (Modrinth o file `.mrpack`): loader,
/// versioni e mod lato server vengono dal pack.
#[tauri::command]
pub async fn server_create_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ModpackServerInput,
) -> CmdResult<ServerInfoView> {
    let installer = state.installer();
    let name = input.name.trim().to_owned();
    // Il pack da Modrinth si scarica (verificato) e poi si cancella.
    let (pack, downloaded) = match (&input.project, &input.file) {
        (Some(project), _) => (
            hyper_content::mrpack::download_from_modrinth(
                &state.modrinth,
                installer.downloader(),
                &state.paths.cache_dir(),
                project,
            )
            .await
            .cmd()?,
            true,
        ),
        (None, Some(file)) => (std::path::PathBuf::from(file), false),
        (None, None) => return Err("Scegli un modpack.".into()),
    };
    let pack_name = hyper_content::mrpack::read_index(&pack)
        .map(|i| i.name)
        .unwrap_or_else(|_| name.clone());
    let request = NewServer {
        name: name.clone(),
        kind: ServerKind::Vanilla,
        version: String::new(),
        memory_mb: input.memory_mb,
        port: input.port,
        keep_running: input.keep_running,
        autostart: input.autostart,
        eula: input.eula,
        loader_version: None,
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
    let source = hyper_server::config::ModpackSource {
        name: pack_name,
        project: input.project.clone(),
    };
    let result = install::create_from_modpack(
        &installer,
        &manager(&state),
        &request,
        &pack,
        source,
        Some(&report),
    )
    .await;
    if downloaded {
        let _ = std::fs::remove_file(&pack);
    }
    let config = result.cmd()?;
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
    // Il tunnel playit va tolto dall'account, non solo dimenticato.
    playit::disable(&manager(&state), &name).await.cmd()?;
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

/// Salva gli automatismi (riavvio giornaliero, backup automatici); li
/// applica l'host anche a launcher chiuso.
#[tauri::command]
pub async fn server_set_schedule(
    state: State<'_, AppState>,
    name: String,
    schedule: hyper_server::config::Schedule,
) -> CmdResult<ServerInfoView> {
    if let Some(at) = &schedule.restart_at
        && hyper_server::schedule::parse_time(at).is_none()
    {
        return Err("Ora del riavvio non valida (usa HH:MM).".into());
    }
    if schedule
        .backup_hours
        .is_some_and(|h| !(1..=168).contains(&h))
    {
        return Err("I backup automatici vanno da ogni ora a ogni settimana.".into());
    }
    if !(1..=100).contains(&schedule.keep_backups) {
        return Err("Si possono tenere da 1 a 100 backup.".into());
    }
    let manager = manager(&state);
    let mut config = manager.load(&name).cmd()?;
    config.schedule = schedule;
    manager.save(&config).cmd()?;
    info(&state, config).await
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
pub async fn server_save_properties(
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
        // Il tunnel playit punta alla porta vecchia: si rifà sulla nuova.
        if config.playit_tunnel.is_some() {
            playit::disable(&manager, &name).await.cmd()?;
            playit::enable(&state.installer(), &state.paths, &manager, &name, |_| {})
                .await
                .cmd()?;
        }
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

/// Cartella dei contenuti (`plugins/` per Paper, `mods/` per i server con
/// loader), loader Modrinth e versione di Minecraft di un server.
fn content_target(
    state: &AppState,
    name: &str,
) -> CmdResult<(std::path::PathBuf, &'static [&'static str], String)> {
    let manager = manager(state);
    let config = manager.load(name).cmd()?;
    let (dir, loaders) = plugins::target(&manager.dir(name).cmd()?, config.kind)
        .ok_or_else(|| "I server Vanilla non hanno né plugin né mod.".to_owned())?;
    Ok((dir, loaders, config.version))
}

#[tauri::command]
pub async fn server_plugins(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<Vec<plugins::InstalledPlugin>> {
    let (dir, _, _) = content_target(&state, &name)?;
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
    let (dir, loaders, version) = content_target(&state, &name)?;
    let installer = state.installer();
    plugins::install(
        &state.modrinth,
        installer.downloader(),
        &dir,
        loaders,
        &project,
        &version,
    )
    .await
    .cmd()
}

/// A server acceso Windows tiene aperti i jar: si possono toccare solo da spento.
async fn plugins_unlocked(state: &AppState, name: &str) -> CmdResult<()> {
    if is_running(state, name).await {
        return Err("Ferma il server per attivare, disattivare o eliminare plugin e mod.".into());
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
    let (dir, _, _) = content_target(&state, &name)?;
    plugins_unlocked(&state, &name).await?;
    plugins::set_enabled(&dir, &file, enabled).cmd()
}

#[tauri::command]
pub async fn server_plugin_delete(
    state: State<'_, AppState>,
    name: String,
    file: String,
) -> CmdResult<()> {
    let (dir, _, _) = content_target(&state, &name)?;
    plugins_unlocked(&state, &name).await?;
    plugins::remove(&dir, &file).cmd()
}

/// Se il server si è fermato con un errore, la causa probabile (dalla
/// console tenuta dall'host); `None` se è acceso o si è fermato bene.
#[tauri::command]
pub async fn server_crash(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<Option<hyper_instances::crash::CrashInfo>> {
    let Ok(chunk) = client(&state).console(&name, 0).await else {
        return Ok(None);
    };
    let last = chunk.lines.last().map(String::as_str).unwrap_or_default();
    // Ultima riga "[Hyper Launcher] Server fermato (codice N)".
    if !last.starts_with("[Hyper Launcher] Server fermato") {
        return Ok(None);
    }
    let failed = !last.ends_with("(codice 0)") && last.contains("(codice");
    let found = hyper_instances::crash::analyze_text(&chunk.lines.join("\n"));
    // Con codice 0 il server si è chiuso "bene", ma una porta occupata o
    // l'EULA lo fermano proprio così: quelle si segnalano lo stesso.
    let fatal_anyway = found
        .as_ref()
        .is_some_and(|c| c.title == "Porta già in uso" || c.title == "EULA non accettata");
    if !failed && !fatal_anyway {
        return Ok(None);
    }
    Ok(Some(found.unwrap_or_else(|| {
        hyper_instances::crash::CrashInfo {
            title: "Il server si è fermato con un errore".to_owned(),
            cause: chunk
                .lines
                .iter()
                .rev()
                .find(|l| l.contains("ERROR") || l.contains("Exception"))
                .cloned(),
            hint: "Guarda le ultime righe della console: di solito indicano la mod o il plugin coinvolto.".to_owned(),
            file: None,
        }
    })))
}

// ---------- Gioca e invito ----------

/// Cosa serve per entrare in un server dal launcher: l'istanza da avviare e
/// l'indirizzo per Quick Play.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayTarget {
    pub instance: String,
    pub join: String,
}

/// Istanza per giocare nel server: quella collegata se esiste ancora,
/// altrimenti ne crea una con la stessa versione e lo stesso loader (per un
/// server da modpack importa lo stesso modpack, con le mod lato client).
#[tauri::command]
pub async fn server_play(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<PlayTarget> {
    let manager = manager(&state);
    let mut config = manager.load(&name).cmd()?;
    let instances = state.instances();
    let join = format!("localhost:{}", config.port);
    if let Some(existing) = config
        .instance
        .as_ref()
        .filter(|i| instances.load(i).is_ok())
    {
        return Ok(PlayTarget {
            instance: existing.clone(),
            join,
        });
    }
    let pack = manager
        .dir(&name)
        .cmd()?
        .join(hyper_server::config::MODPACK_COPY);
    let instance = if config.modpack.is_some() && pack.is_file() {
        let report = crate::content::progress_emitter(&app, "");
        hyper_content::mrpack::import(
            &instances,
            state.installer().downloader(),
            &pack,
            Some(&report),
        )
        .await
        .cmd()?
    } else {
        let loader = config
            .kind
            .loader()
            .zip(config.loader_version.clone())
            .map(|(kind, version)| hyper_installer::LoaderSpec { kind, version });
        let base = format!("{name} (server)");
        let unique = instances.unique_name(&base).cmd()?;
        instances.create(&unique, &config.version, loader).cmd()?
    };
    config.instance = Some(instance.config.name.clone());
    manager.save(&config).cmd()?;
    Ok(PlayTarget {
        instance: instance.config.name,
        join,
    })
}

// ---------- Accesso da internet (playit.gg) ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayitStatus {
    /// playit.gg è collegato a questo PC (chiave nel keyring).
    pub connected: bool,
}

#[tauri::command]
pub fn playit_status() -> CmdResult<PlayitStatus> {
    Ok(PlayitStatus {
        connected: playit::secret().cmd()?.is_some(),
    })
}

/// Collega playit.gg: apre il browser sulla pagina di approvazione e
/// aspetta (evento `playit-claim` con lo stato) finché l'utente approva.
#[tauri::command]
pub async fn playit_connect(app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let code = playit::claim_code();
    app.opener()
        .open_url(playit::claim_url(&code), None::<&str>)
        .cmd()?;
    playit::Client::new()
        .cmd()?
        .claim(&code, |status| {
            let _ = app.emit("playit-claim", status);
        })
        .await
        .cmd()
}

/// Scollega playit.gg da questo PC, togliendo prima i tunnel dei server.
#[tauri::command]
pub async fn playit_disconnect(state: State<'_, AppState>) -> CmdResult<()> {
    playit::disconnect(&manager(&state)).await.cmd()
}

#[derive(Clone, Serialize)]
struct PlayitProgress<'a> {
    name: &'a str,
    stage: playit::EnableStage,
}

/// Rende il server raggiungibile da internet (evento `playit-progress`).
#[tauri::command]
pub async fn server_playit_enable(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<ServerInfoView> {
    let manager = manager(&state);
    playit::enable(&state.installer(), &state.paths, &manager, &name, |stage| {
        let _ = app.emit("playit-progress", PlayitProgress { name: &name, stage });
    })
    .await
    .cmd()?;
    info(&state, manager.load(&name).cmd()?).await
}

#[tauri::command]
pub async fn server_playit_disable(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<ServerInfoView> {
    let manager = manager(&state);
    playit::disable(&manager, &name).await.cmd()?;
    info(&state, manager.load(&name).cmd()?).await
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
