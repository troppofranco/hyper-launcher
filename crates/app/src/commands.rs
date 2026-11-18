//! Comandi invocati dall'interfaccia. Sono sottili: le regole stanno nei
//! crate del core, gli stessi usati dalla CLI.

use std::path::{Path, PathBuf};

use hyper_auth::{Account, AccountStore, AuthError, KeyringStore, MsaClient};
use hyper_core::{Config, SyncConfig};
use hyper_installer::{LoaderKind, VersionKind, self_update};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, CmdResult, OrCmd};
use crate::views::{
    AccountView, AppInfo, InstanceView, NewsView, SessionView, UpdateView, VersionView,
    VersionsView, WorldView,
};

// ---------- App ----------

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        build: self_update::current_build(),
        data_dir: state.paths.root().display().to_string(),
    }
}

// ---------- Istanze ----------

#[tauri::command]
pub fn list_instances(state: State<'_, AppState>) -> CmdResult<Vec<InstanceView>> {
    let instances = state.instances().list().cmd()?;
    Ok(instances.iter().map(InstanceView::new).collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInstance {
    /// Vuoto = nome automatico.
    pub name: String,
    pub version: String,
    /// Assente = vanilla.
    pub loader: Option<LoaderKind>,
}

#[tauri::command]
pub async fn create_instance(
    state: State<'_, AppState>,
    input: NewInstance,
) -> CmdResult<InstanceView> {
    let loader = match input.loader {
        Some(kind) => Some(
            state
                .installer()
                .latest_loader(kind, &input.version)
                .await
                .cmd()?,
        ),
        None => None,
    };
    let instance = state
        .instances()
        .create_named(&input.name, &input.version, loader)
        .cmd()?;
    Ok(InstanceView::new(&instance))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSettings {
    pub memory_mb: Option<u32>,
    pub java_path: Option<String>,
    pub jvm_args: String,
    pub sync_settings: bool,
}

#[tauri::command]
pub fn update_instance(
    state: State<'_, AppState>,
    name: String,
    settings: InstanceSettings,
) -> CmdResult<InstanceView> {
    let mut instance = state.instances().load(&name).cmd()?;
    instance.config.memory_mb = settings.memory_mb;
    instance.config.java_path = settings
        .java_path
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    instance.config.jvm_args = settings
        .jvm_args
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    instance.config.sync_settings = settings.sync_settings;
    // Valida subito, così la UI mostra l'errore invece di fallire all'avvio.
    instance.config.effective_memory_mb(&state.config()).cmd()?;
    instance.save().cmd()?;
    Ok(InstanceView::new(&instance))
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, name: String) -> CmdResult<()> {
    if state.is_busy(&name) {
        return Err("L'istanza è in uso: chiudi il gioco prima di eliminarla.".into());
    }
    let manager = state.instances();
    // Cancellare gigabyte di mondi richiede tempo: fuori dal thread della UI.
    tokio::task::spawn_blocking(move || manager.delete(&name))
        .await
        .cmd()?
        .cmd()
}

// ---------- Versioni ----------

#[tauri::command]
pub async fn list_versions(state: State<'_, AppState>) -> CmdResult<VersionsView> {
    let manifest = state.installer().fetch_manifest().await.cmd()?;
    Ok(VersionsView {
        latest_release: manifest.latest.release.clone(),
        latest_snapshot: manifest.latest.snapshot.clone(),
        versions: manifest
            .versions
            .iter()
            .filter(|v| matches!(v.kind, VersionKind::Release | VersionKind::Snapshot))
            .map(|v| VersionView {
                id: v.id.clone(),
                kind: v.kind.as_str(),
                release_time: v.release_time.clone(),
            })
            .collect(),
    })
}

/// Versioni stabili del loader per una versione di gioco (vuoto = non supportata).
#[tauri::command]
pub async fn loader_versions(
    state: State<'_, AppState>,
    loader: LoaderKind,
    game: String,
) -> CmdResult<Vec<String>> {
    let versions = state
        .installer()
        .loader_versions(loader, &game)
        .await
        .cmd()?;
    Ok(versions
        .into_iter()
        .filter(|v| v.stable)
        .map(|v| v.version)
        .collect())
}

// ---------- Account ----------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeEvent {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

/// Carica `accounts.toml`, applica la modifica e restituisce l'elenco
/// aggiornato per la UI. Ogni modifica salva il file da sé.
fn edit_accounts(
    state: &AppState,
    edit: impl FnOnce(&mut AccountStore, &Path, &KeyringStore) -> CmdResult<()>,
) -> CmdResult<Vec<AccountView>> {
    let path = state.paths.accounts_file();
    let mut store = AccountStore::load(&path).cmd()?;
    edit(&mut store, &path, &KeyringStore::default())?;
    Ok(store
        .accounts()
        .iter()
        .map(|a| AccountView::new(a, store.is_default(a)))
        .collect())
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> CmdResult<Vec<AccountView>> {
    edit_accounts(&state, |_, _, _| Ok(()))
}

/// Aggiunge un account offline senza cambiare quello attivo (diventa
/// predefinito solo se è il primo).
#[tauri::command]
pub fn add_offline_account(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<Vec<AccountView>> {
    let account = Account::offline(name.trim()).cmd()?;
    edit_accounts(&state, |store, path, secrets| {
        store.add_and_save(path, account, secrets).cmd()
    })
}

/// Login Microsoft con device code: emette `msa-code` con il codice da
/// mostrare, apre la pagina di Microsoft nel browser e attende la conferma.
/// L'account appena collegato diventa quello attivo.
#[tauri::command]
pub async fn add_microsoft_account(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Vec<AccountView>> {
    let msa = MsaClient::from_config(&state.config())
        .cmd()?
        .ok_or(AuthError::MissingClientId)
        .cmd()?;
    let code = msa.start_device_login().await.cmd()?;
    app.emit(
        "msa-code",
        DeviceCodeEvent {
            user_code: code.user_code.clone(),
            verification_uri: code.verification_uri.clone(),
            expires_in: code.expires_in,
        },
    )
    .cmd()?;
    let _ = app
        .opener()
        .open_url(code.verification_uri.as_str(), None::<&str>);

    let account = hyper_auth::login_microsoft(&msa, &KeyringStore::default(), &code)
        .await
        .cmd()?;
    let name = account.name().to_owned();
    edit_accounts(&state, |store, path, secrets| {
        store.add_and_save(path, account, secrets).cmd()?;
        store.set_default(&name).cmd()?;
        store.save(path).cmd()
    })
}

#[tauri::command]
pub fn set_default_account(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<Vec<AccountView>> {
    edit_accounts(&state, |store, path, _| {
        store.set_default(&name).cmd()?;
        store.save(path).cmd()
    })
}

#[tauri::command]
pub fn remove_account(state: State<'_, AppState>, name: String) -> CmdResult<Vec<AccountView>> {
    edit_accounts(&state, |store, path, secrets| {
        store.remove_and_save(path, &name, secrets).cmd().map(drop)
    })
}

// ---------- Impostazioni ----------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub default_memory_mb: u32,
    pub java_path: String,
    pub msa_client_id: String,
    pub accent: String,
    pub minimize_on_launch: bool,
    pub show_console: bool,
    pub theme: String,
    pub language: String,
    pub setup_done: bool,
    pub sync: SyncConfig,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsView {
    let c = state.config();
    SettingsView {
        default_memory_mb: c.default_memory_mb,
        java_path: c
            .java_path
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        msa_client_id: c.msa_client_id.unwrap_or_default(),
        accent: c.accent,
        minimize_on_launch: c.minimize_on_launch,
        show_console: c.show_console,
        theme: c.theme,
        language: c.language,
        setup_done: c.setup_done,
        sync: c.sync,
    }
}

/// Salva le impostazioni; i controlli (memoria minima, colore) stanno in
/// `Config::validate`.
#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: SettingsView) -> CmdResult<()> {
    let non_empty = |s: String| Some(s.trim().to_owned()).filter(|s| !s.is_empty());
    let config = Config {
        default_memory_mb: settings.default_memory_mb,
        java_path: non_empty(settings.java_path).map(PathBuf::from),
        msa_client_id: non_empty(settings.msa_client_id),
        accent: settings.accent.trim().to_owned(),
        minimize_on_launch: settings.minimize_on_launch,
        show_console: settings.show_console,
        theme: settings.theme,
        language: settings.language,
        setup_done: settings.setup_done,
        sync: settings.sync,
        ..state.config()
    };
    state.save_config(config).cmd()
}

// ---------- Statistiche ----------

/// Partite degli ultimi `days` giorni di tutte le istanze; la UI le
/// raggruppa per giorno nel fuso orario locale.
#[tauri::command]
pub async fn recent_sessions(state: State<'_, AppState>, days: u64) -> CmdResult<Vec<SessionView>> {
    let since = hyper_core::unix_now().saturating_sub(days.min(366) * 86_400);
    let manager = state.instances();
    // Lettura da disco fuori dal thread dell'interfaccia.
    tauri::async_runtime::spawn_blocking(move || sessions_since(&manager, since))
        .await
        .cmd()?
}

fn sessions_since(
    manager: &hyper_instances::InstanceManager,
    since: u64,
) -> CmdResult<Vec<SessionView>> {
    let instances = manager.list().cmd()?;
    let mut sessions: Vec<SessionView> = instances
        .iter()
        // Chi non gioca da prima della finestra non ha sessioni utili.
        .filter(|i| i.config.last_played.is_some_and(|t| t >= since))
        .flat_map(|i| {
            i.sessions()
                .into_iter()
                .filter(|s| s.end >= since)
                .map(|s| SessionView {
                    instance: i.config.name.clone(),
                    start: s.start,
                    end: s.end,
                })
        })
        .collect();
    sessions.sort_by_key(|s| s.start);
    Ok(sessions)
}

// ---------- Mondi, cartelle ----------

#[tauri::command]
pub fn list_worlds(state: State<'_, AppState>, name: String) -> CmdResult<Vec<WorldView>> {
    let instance = state.instances().load(&name).cmd()?;
    Ok(instance.worlds().into_iter().map(WorldView::from).collect())
}

/// Apre in Esplora risorse una cartella dell'istanza (`game`, `mods`,
/// `saves`, `screenshots`, `resourcepacks`, `shaderpacks`) o, senza
/// istanza, la cartella dati.
#[tauri::command]
pub fn open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    name: Option<String>,
    folder: String,
) -> CmdResult<()> {
    let path = match name {
        None => state.paths.root().to_path_buf(),
        Some(name) => {
            let instance = state.instances().load(&name).cmd()?;
            match folder.as_str() {
                "game" => instance.game_dir(),
                "mods" => instance.mods_dir(),
                "saves" => instance.saves_dir(),
                "screenshots" | "resourcepacks" | "shaderpacks" => {
                    instance.game_dir().join(&folder)
                }
                _ => return Err("Cartella non valida.".into()),
            }
        }
    };
    hyper_core::fs::create_dir_all(&path).cmd()?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .cmd()
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    // Solo pagine web: mai file locali o protocolli arbitrari dalla UI.
    if !url.starts_with("https://") {
        return Err("Link non valido.".into());
    }
    app.opener().open_url(url, None::<&str>).cmd()
}

// ---------- Notizie ----------

const NEWS_URL: &str = "https://launchercontent.mojang.com/v2/news.json";
const NEWS_HOST: &str = "https://launchercontent.mojang.com";

#[derive(Deserialize)]
struct NewsFeed {
    entries: Vec<NewsEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewsEntry {
    title: String,
    category: String,
    date: String,
    #[serde(default)]
    text: String,
    news_page_image: Option<NewsImage>,
    play_page_image: Option<NewsImage>,
    read_more_link: String,
}

#[derive(Deserialize)]
struct NewsImage {
    url: String,
}

#[tauri::command]
pub async fn news(state: State<'_, AppState>) -> CmdResult<Vec<NewsView>> {
    let bytes = state
        .installer()
        .downloader()
        .get_bytes(NEWS_URL)
        .await
        .cmd()?;
    let feed: NewsFeed = serde_json::from_slice(&bytes).cmd()?;
    Ok(feed
        .entries
        .into_iter()
        .filter(|e| e.category == "Minecraft: Java Edition")
        .take(8)
        .map(|e| NewsView {
            image: e
                .play_page_image
                .or(e.news_page_image)
                .filter(|i| i.url.starts_with('/'))
                .map(|i| format!("{NEWS_HOST}{}", i.url)),
            title: e.title,
            date: e.date,
            text: e.text,
            url: e.read_more_link,
        })
        .collect())
}

// ---------- Aggiornamenti ----------

#[tauri::command]
pub async fn check_update() -> CmdResult<UpdateView> {
    Ok(UpdateView {
        current: self_update::current_build(),
        available: self_update::available().await.cmd()?.map(|u| u.build),
    })
}

/// Scarica e verifica il setup più recente, lo avvia in modo silenzioso e
/// chiude l'app: l'installer la riaprirà aggiornata.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    // Uscire ora interromperebbe un'installazione o la registrazione del
    // tempo di gioco.
    if state.any_busy() {
        return Err(
            "Chiudi il gioco (o attendi la fine dell'installazione) prima di aggiornare.".into(),
        );
    }
    if self_update::current_build().is_none() {
        return Err("Questa è una build di sviluppo: niente aggiornamenti.".into());
    }
    let installer = state.installer();
    match self_update::update_to_latest(installer.downloader(), &state.paths.updates_dir(), true)
        .await
        .cmd()?
    {
        Some(_) => {
            app.exit(0);
            Ok(())
        }
        None => Err("Hyper Launcher è già aggiornato.".into()),
    }
}

/// Mostra la finestra principale (es. dopo che il gioco si chiude).
pub fn restore_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
