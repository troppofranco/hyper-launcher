//! Comandi del catalogo Modrinth: ricerca, dettaglio, installazione,
//! contenuti installati con aggiornamenti, modpack `.mrpack`. La logica sta
//! in `hyper-content`, la stessa della CLI.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hyper_content::library::{installed, update};
use hyper_content::{ContentKind, SearchQuery, SearchSort, install, mrpack};
use hyper_installer::Progress;
use hyper_instances::ContentDir;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::{AppState, CmdResult, OrCmd};
use crate::views::{ContentView, ExportView, InstanceView, PlanView, ProjectView, SearchView};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    pub text: String,
    pub kind: ContentKind,
    /// Mostra solo ciò che è compatibile con questa istanza.
    pub instance: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub sort: SearchSort,
    #[serde(default)]
    pub offset: u32,
}

#[tauri::command]
pub async fn content_search(
    state: State<'_, AppState>,
    input: SearchInput,
) -> CmdResult<SearchView> {
    let mut query = SearchQuery::new(input.text, input.kind);
    if let Some(name) = input.instance {
        query.game_version = Some(state.instances().load(&name).cmd()?.config.version);
    }
    query.categories = input.categories;
    query.sort = input.sort;
    query.offset = input.offset;
    let results = state.modrinth.search(&query).await.cmd()?;
    Ok(SearchView::from(results))
}

#[tauri::command]
pub async fn content_project(state: State<'_, AppState>, id: String) -> CmdResult<ProjectView> {
    Ok(ProjectView::from(state.modrinth.project(&id).await.cmd()?))
}

/// Cosa verrebbe installato (progetto + dipendenze), senza scaricare.
#[tauri::command]
pub async fn content_plan(
    state: State<'_, AppState>,
    instance: String,
    project: String,
    kind: ContentKind,
    world: Option<String>,
) -> CmdResult<PlanView> {
    let instance = state.instances().load(&instance).cmd()?;
    let plan = install::plan(&state.modrinth, &instance, &project, kind, world.as_deref())
        .await
        .cmd()?;
    Ok(PlanView::from(&plan))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContentProgress<'a> {
    /// Istanza interessata (`""` durante l'import di un modpack).
    instance: &'a str,
    done: usize,
    total: usize,
}

/// Callback di avanzamento verso la UI, al massimo ogni 100 ms.
fn progress_emitter<'a>(app: &'a AppHandle, instance: &'a str) -> impl Fn(Progress) + Sync + 'a {
    let last = Mutex::new(None::<Instant>);
    move |p: Progress| {
        let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
        let due = last.is_none_or(|t| t.elapsed() >= Duration::from_millis(100));
        if due || p.done == p.total {
            *last = Some(Instant::now());
            let _ = app.emit(
                "content-progress",
                ContentProgress {
                    instance,
                    done: p.done,
                    total: p.total,
                },
            );
        }
    }
}

/// Installa un progetto e le sue dipendenze; restituisce quanti file.
#[tauri::command]
pub async fn content_install(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    project: String,
    kind: ContentKind,
    world: Option<String>,
) -> CmdResult<usize> {
    let target = state.instances().load(&instance).cmd()?;
    let plan = install::plan(&state.modrinth, &target, &project, kind, world.as_deref())
        .await
        .cmd()?;
    let report = progress_emitter(&app, &instance);
    plan.execute(&target, state.installer().downloader(), Some(&report))
        .await
        .cmd()?;
    Ok(plan.files.len())
}

/// Contenuti installati. Con la rete li riconosce su Modrinth (nome, icona,
/// aggiornamenti); senza rete restituisce comunque l'elenco dei file.
#[tauri::command]
pub async fn content_installed(
    state: State<'_, AppState>,
    instance: String,
    dir: ContentDir,
    check_updates: bool,
) -> CmdResult<Vec<ContentView>> {
    let target = state.instances().load(&instance).cmd()?;
    match installed(&state.modrinth, &target, dir, check_updates).await {
        Ok(items) => Ok(items.into_iter().map(ContentView::from).collect()),
        Err(e) => {
            tracing::warn!(error = %e, "Modrinth unavailable, showing plain files");
            Ok(target
                .content_files(dir)
                .into_iter()
                .map(ContentView::from)
                .collect())
        }
    }
}

#[tauri::command]
pub fn set_content_enabled(
    state: State<'_, AppState>,
    instance: String,
    dir: ContentDir,
    file: String,
    enabled: bool,
) -> CmdResult<()> {
    state
        .instances()
        .load(&instance)
        .cmd()?
        .set_content_enabled(dir, &file, enabled)
        .cmd()
}

#[tauri::command]
pub fn delete_content(
    state: State<'_, AppState>,
    instance: String,
    dir: ContentDir,
    file: String,
) -> CmdResult<()> {
    state
        .instances()
        .load(&instance)
        .cmd()?
        .delete_content(dir, &file)
        .cmd()
}

/// Aggiorna i file indicati (vuoto = tutti); restituisce quanti.
#[tauri::command]
pub async fn content_update(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    dir: ContentDir,
    files: Vec<String>,
) -> CmdResult<usize> {
    let target = state.instances().load(&instance).cmd()?;
    let report = progress_emitter(&app, &instance);
    update(
        &state.modrinth,
        state.installer().downloader(),
        &target,
        dir,
        &files,
        Some(&report),
    )
    .await
    .cmd()
}

/// Installa l'ultima versione di un modpack Modrinth come nuova istanza.
#[tauri::command]
pub async fn pack_install(
    app: AppHandle,
    state: State<'_, AppState>,
    project: String,
) -> CmdResult<InstanceView> {
    let report = progress_emitter(&app, "");
    let instance = mrpack::install_from_modrinth(
        &state.modrinth,
        &state.instances(),
        state.installer().downloader(),
        &state.paths.cache_dir(),
        &project,
        Some(&report),
    )
    .await
    .cmd()?;
    Ok(InstanceView::new(&instance))
}

/// Chiede un file `.mrpack` e lo importa; `None` se l'utente annulla.
#[tauri::command]
pub async fn pack_import(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<InstanceView>> {
    let Some(file) = app
        .dialog()
        .file()
        .set_title("Importa un modpack")
        .add_filter("Modpack Modrinth", &["mrpack"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().cmd()?;
    let report = progress_emitter(&app, "");
    let instance = mrpack::import(
        &state.instances(),
        state.installer().downloader(),
        &path,
        Some(&report),
    )
    .await
    .cmd()?;
    Ok(Some(InstanceView::new(&instance)))
}

/// Chiede dove salvare ed esporta l'istanza in `.mrpack`; `None` se
/// l'utente annulla.
#[tauri::command]
pub async fn pack_export(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
) -> CmdResult<Option<ExportView>> {
    let target = state.instances().load(&instance).cmd()?;
    let Some(file) = app
        .dialog()
        .file()
        .set_title("Esporta come modpack")
        .set_file_name(format!("{instance}.mrpack"))
        .add_filter("Modpack Modrinth", &["mrpack"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let mut path: PathBuf = file.into_path().cmd()?;
    if path.extension().is_none_or(|e| e != "mrpack") {
        path.set_extension("mrpack");
    }
    let summary = mrpack::export(&state.modrinth, &target, &path)
        .await
        .cmd()?;
    Ok(Some(ExportView {
        path: path.display().to_string(),
        linked: summary.linked,
        bundled: summary.bundled,
    }))
}
