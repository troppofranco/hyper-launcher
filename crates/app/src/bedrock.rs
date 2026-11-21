//! Installazioni Bedrock: stato del gioco, contenuti e avvio (vedi
//! `hyper_bedrock` per lo scambio delle cartelle).

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use hyper_bedrock::{DataDirs, Game, Imported, Location, PackKind, Scope};
use hyper_instances::Instance;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, CmdResult, OrCmd};

/// Impostazioni e tasti del gioco, nella cartella del profilo Xbox.
const OPTIONS: &str = "minecraftpe/options.txt";

fn dirs() -> CmdResult<DataDirs> {
    DataDirs::locate().ok_or_else(|| "Cartella dati di Windows non trovata.".into())
}

/// Nome dell'installazione i cui dati sono ora nelle cartelle del gioco.
fn active(state: &AppState) -> CmdResult<Option<String>> {
    Ok(hyper_bedrock::State::load(state.paths.root()).cmd()?.active)
}

/// Dove stanno adesso i contenuti di un'installazione Bedrock.
pub fn location(state: &AppState, instance: &Instance) -> CmdResult<Location> {
    if !instance.is_bedrock() {
        return Err("Non è un'installazione Bedrock.".into());
    }
    let is_active = active(state)?.as_deref() == Some(instance.config.name.as_str());
    Ok(Location::new(dirs()?, instance.bedrock_dir(), is_active))
}

/// Il gioco aperto blocca le modifiche all'installazione attiva (i suoi file
/// sono quelli che il gioco sta usando), un avvio in corso quelle di tutte.
/// La guardia restituita va tenuta finché la modifica non è finita.
pub fn ensure_editable(state: &AppState, instance: &Instance) -> CmdResult<(Location, EditGuard)> {
    let guard = EditGuard::acquire()?;
    let location = location(state, instance)?;
    if location.is_active() && hyper_bedrock::running() {
        return Err("Chiudi Minecraft Bedrock prima di modificare questa installazione.".into());
    }
    Ok((location, guard))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BedrockStatus {
    pub installed: bool,
    pub version: Option<String>,
    /// Installazione che occupa ora le cartelle del gioco.
    pub active: Option<String>,
    /// `false` finché il gioco non ha creato la cartella del profilo Xbox.
    pub profile: bool,
    pub running: bool,
}

#[tauri::command]
pub async fn bedrock_status(state: State<'_, AppState>) -> CmdResult<BedrockStatus> {
    let active = active(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let game = Game::detect();
        BedrockStatus {
            installed: game.is_some(),
            version: game.map(|g| g.version),
            active,
            profile: DataDirs::locate().is_some_and(|d| d.user.is_some()),
            running: hyper_bedrock::running(),
        }
    })
    .await
    .cmd()
}

/// Crea un'installazione Bedrock. La prima "adotta" ciò che c'è già nel
/// gioco: diventa l'attiva senza spostare nulla.
pub fn create(state: &AppState, name: &str) -> CmdResult<Instance> {
    let game = Game::detect().ok_or("Minecraft Bedrock non è installato.")?;
    let manager = state.instances();
    let instance = manager.create_bedrock(name, &game.version).cmd()?;
    let adopted = match active(state)? {
        // L'attiva registrata esiste ancora: la nuova parte vuota.
        Some(current) if manager.load(&current).is_ok() => false,
        _ => true,
    };
    if adopted {
        hyper_bedrock::set_active(state.paths.root(), Some(&instance.config.name)).cmd()?;
    }
    Ok(instance)
}

/// Rifiuta di eliminare l'installazione attiva: i suoi dati sono quelli
/// del gioco.
pub fn ensure_deletable(state: &AppState, instance: &Instance) -> CmdResult<()> {
    if instance.is_bedrock() && active(state)?.as_deref() == Some(instance.config.name.as_str()) {
        return Err(
            "Questa installazione è quella in uso dal gioco: avvia prima un'altra installazione Bedrock, poi eliminala."
                .into(),
        );
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackView {
    #[serde(flatten)]
    pub pack: hyper_bedrock::Pack,
    pub icon: Option<String>,
}

#[tauri::command]
pub async fn bedrock_packs(
    state: State<'_, AppState>,
    name: String,
    kind: PackKind,
) -> CmdResult<Vec<PackView>> {
    let instance = state.instances().load(&name).cmd()?;
    let location = location(&state, &instance)?;
    tauri::async_runtime::spawn_blocking(move || {
        hyper_bedrock::list(&location, kind)
            .into_iter()
            .map(|pack| PackView {
                icon: pack.icon.as_deref().and_then(|bytes| {
                    Some(crate::custom::data_url(
                        hyper_instances::image_mime(bytes)?,
                        bytes,
                    ))
                }),
                pack,
            })
            .collect()
    })
    .await
    .cmd()
}

/// Importa file scaricati (`path`, es. trascinato nella finestra) o, senza
/// percorso, quelli scelti nel selettore.
#[tauri::command]
pub async fn bedrock_import(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    paths: Option<Vec<String>>,
) -> CmdResult<Vec<Imported>> {
    let instance = state.instances().load(&name).cmd()?;
    if state.is_busy(&name) {
        return Err("Chiudi il gioco prima di aggiungere contenuti.".into());
    }
    let files: Vec<PathBuf> = match paths {
        Some(paths) => paths.into_iter().map(PathBuf::from).collect(),
        None => {
            let picked = app
                .dialog()
                .file()
                .add_filter(
                    "Contenuti di Minecraft Bedrock",
                    &["mcpack", "mcaddon", "mcworld", "mctemplate", "zip"],
                )
                .blocking_pick_files();
            picked
                .unwrap_or_default()
                .into_iter()
                .filter_map(|f| f.into_path().ok())
                .collect()
        }
    };
    // Dopo la scelta dei file: il selettore aperto non deve bloccare gli avvii.
    let (location, guard) = ensure_editable(&state, &instance)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let mut all = Vec::new();
        for file in files {
            all.extend(hyper_bedrock::import(&location, &file).cmd()?);
        }
        Ok(all)
    })
    .await
    .cmd()?
}

#[tauri::command]
pub async fn bedrock_delete(
    state: State<'_, AppState>,
    name: String,
    kind: PackKind,
    scope: Scope,
    folder: String,
) -> CmdResult<()> {
    let instance = state.instances().load(&name).cmd()?;
    if state.is_busy(&name) {
        return Err("Chiudi il gioco prima di eliminare contenuti.".into());
    }
    let (location, guard) = ensure_editable(&state, &instance)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        hyper_bedrock::delete_pack(&location, kind, scope, &folder).cmd()
    })
    .await
    .cmd()?
}

/// Apre in Esplora risorse la cartella di un tipo di contenuto.
#[tauri::command]
pub fn bedrock_open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    kind: PackKind,
) -> CmdResult<()> {
    let instance = state.instances().load(&name).cmd()?;
    let location = location(&state, &instance)?;
    let (scope, folder) = match kind {
        PackKind::Resource => (Scope::Shared, "resource_packs"),
        PackKind::Behavior => (Scope::Shared, "behavior_packs"),
        PackKind::Skin => (Scope::Shared, "skin_packs"),
        PackKind::World => (Scope::User, "minecraftWorlds"),
        PackKind::Template => (Scope::User, "world_templates"),
    };
    let path = location
        .folder(scope, folder)
        .ok_or("Apri Minecraft Bedrock una volta, poi riprova.")?;
    hyper_core::fs::create_dir_all(&path).cmd()?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .cmd()
}

/// Avvio di un'installazione Bedrock: scambio delle cartelle se serve,
/// sincronizzazione, gioco, attesa della chiusura. Restituisce quando il
/// gioco si chiude.
pub async fn run(app: &AppHandle, name: &str, on_running: impl FnOnce()) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    let config = state.config();
    let root = state.paths.root().to_path_buf();
    let instance = state.instances().load(name)?;
    // Il gioco è uno solo: mentre un'installazione aspetta che si apra (o ci
    // si gioca) un'altra non deve scambiare le cartelle.
    let _running = RunGuard::acquire().ok_or_else(|| {
        anyhow::anyhow!(
            "C'è già un'installazione Bedrock in avvio o in gioco, o si stanno modificando i suoi contenuti: riprova tra poco."
        )
    })?;
    let Some(game) = Game::detect() else {
        anyhow::bail!("Minecraft Bedrock non è installato.");
    };
    // Bedrock si aggiorna da solo: la versione mostrata segue il gioco.
    let mut instance = instance;
    if instance.config.version != game.version {
        instance.config.version = game.version;
        if let Err(e) = instance.save() {
            tracing::warn!(error = %e, "cannot save the bedrock version");
        }
    }
    if hyper_bedrock::running() {
        anyhow::bail!("Minecraft Bedrock è già aperto: chiudilo e riprova.");
    }
    let _lock = instance.try_lock()?;
    let dirs = DataDirs::locate().ok_or_else(|| anyhow::anyhow!("Cartella dati non trovata."))?;

    let sync_on = config.sync.enabled && instance.config.sync_settings;
    let shared = state.paths.shared_bedrock_options_file();

    // Le cartelle del gioco passano a questa installazione.
    let current = hyper_bedrock::State::load(&root)?.active;
    if current.as_deref() != Some(name) {
        let outgoing = current
            .as_deref()
            .and_then(|c| state.instances().load(c).ok());
        // Prima volta con la sincronizzazione: le impostazioni di chi lascia
        // il gioco (magari usato a lungo fuori dal launcher) diventano
        // quelle condivise, così arrivano anche alla nuova installazione.
        if sync_on
            && !shared.exists()
            && outgoing.as_ref().is_some_and(|o| o.config.sync_settings)
            && let Some(options) = dirs.game_folder(Scope::User, OPTIONS)
            && let Err(e) = hyper_sync::push_in(
                hyper_sync::Edition::Bedrock,
                &options,
                &shared,
                &config.sync,
                None,
            )
        {
            tracing::warn!(error = %e, "bedrock settings sync (seed) failed");
        }
        let from_storage = outgoing.map(|i| i.bedrock_dir());
        let (root, dirs, to, to_storage) = (
            root.clone(),
            dirs.clone(),
            name.to_owned(),
            instance.bedrock_dir(),
        );
        tokio::task::spawn_blocking(move || {
            hyper_bedrock::swap(&root, &dirs, from_storage, &to, to_storage)
        })
        .await??;
    }

    // Dopo lo scambio le impostazioni di questa installazione sono nel gioco.
    let options = dirs.game_folder(Scope::User, OPTIONS);
    let baseline = match (&options, sync_on) {
        (Some(options), true) => {
            hyper_sync::pull_in(hyper_sync::Edition::Bedrock, &shared, options, &config.sync)
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "bedrock settings sync (pull) failed");
                    None
                })
        }
        _ => None,
    };

    open_game(app).await?;
    let started = hyper_core::unix_now();
    on_running();
    hyper_bedrock::wait_for_exit().await;
    let ended = hyper_core::unix_now();
    *LAST_EXIT.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());

    if let Err(e) = instance.record_session(started, ended) {
        tracing::warn!(error = %e, "cannot record the bedrock session");
    }
    // Il profilo Xbox può essere comparso solo ora (primo avvio del gioco).
    let options =
        options.or_else(|| DataDirs::locate().and_then(|d| d.game_folder(Scope::User, OPTIONS)));
    if sync_on
        && let Some(options) = options
        && let Err(e) = hyper_sync::push_in(
            hyper_sync::Edition::Bedrock,
            &options,
            &shared,
            &config.sync,
            baseline.as_ref(),
        )
    {
        tracing::warn!(error = %e, "bedrock settings sync (push) failed");
    }
    Ok(())
}

/// Quando il gioco avviato dal launcher si è chiuso l'ultima volta.
static LAST_EXIT: Mutex<Option<Instant>> = Mutex::new(None);

/// Dopo una chiusura, Windows (Gaming Services) ignora per circa 30 secondi
/// le richieste di riaprire il gioco: l'aiutante di avvio resta bloccato per
/// un paio di minuti senza aprire nulla. Misurato: 10 s non bastano, 30 sì.
const RELAUNCH_COOLDOWN: Duration = Duration::from_secs(30);

/// Apre il gioco e aspetta che compaia. Se non si apre (es. chiuso da poco
/// fuori dal launcher) aspetta che l'aiutante bloccato se ne vada e riprova
/// una volta.
async fn open_game(app: &AppHandle) -> anyhow::Result<()> {
    let wait = LAST_EXIT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .and_then(|t| RELAUNCH_COOLDOWN.checked_sub(t.elapsed()));
    if let Some(wait) = wait {
        tokio::time::sleep(wait).await;
    }
    for attempt in 0..2 {
        hyper_bedrock::wait_for_helper(Duration::from_secs(180)).await;
        app.opener()
            .open_url(hyper_bedrock::LAUNCH_URI, None::<&str>)
            .map_err(|e| anyhow::anyhow!("impossibile aprire Minecraft Bedrock: {e}"))?;
        match hyper_bedrock::wait_for_start(Duration::from_secs(45)).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt == 1 => return Err(e.into()),
            Err(_) => tracing::warn!("bedrock did not start, retrying"),
        }
    }
    unreachable!("il secondo tentativo restituisce sempre")
}

/// Cosa sta succedendo alle cartelle Bedrock: un avvio (che può scambiarle)
/// esclude le modifiche ai contenuti e viceversa, così un'importazione non
/// scrive in una cartella mentre viene spostata (o in quella sbagliata).
struct Activity {
    launching: bool,
    edits: usize,
}

static ACTIVITY: Mutex<Activity> = Mutex::new(Activity {
    launching: false,
    edits: 0,
});

fn activity() -> std::sync::MutexGuard<'static, Activity> {
    ACTIVITY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Un solo avvio Bedrock alla volta (rilasciato quando `run` finisce).
struct RunGuard;

impl RunGuard {
    fn acquire() -> Option<Self> {
        let mut activity = activity();
        if activity.launching || activity.edits > 0 {
            return None;
        }
        activity.launching = true;
        Some(Self)
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        activity().launching = false;
    }
}

/// Modifica in corso ai contenuti di un'installazione Bedrock.
pub struct EditGuard;

impl EditGuard {
    fn acquire() -> CmdResult<Self> {
        let mut activity = activity();
        if activity.launching {
            return Err(
                "Un'installazione Bedrock è in avvio o in gioco: aspetta che il gioco si chiuda."
                    .into(),
            );
        }
        activity.edits += 1;
        Ok(Self)
    }
}

impl Drop for EditGuard {
    fn drop(&mut self) {
        activity().edits -= 1;
    }
}

/// Completa uno scambio di cartelle interrotto (es. PC spento a metà).
pub fn recover(state: &AppState) {
    let Some(dirs) = DataDirs::locate() else {
        return;
    };
    if let Err(e) = hyper_bedrock::recover(state.paths.root(), &dirs) {
        tracing::error!(error = %e, "cannot complete the interrupted bedrock swap");
    }
}
