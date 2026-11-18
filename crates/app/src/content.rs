//! Comandi del catalogo Modrinth: ricerca, dettaglio, installazione,
//! contenuti installati con aggiornamenti, modpack `.mrpack`. La logica sta
//! in `hyper-content`, la stessa della CLI.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hyper_content::curseforge::{CfMod, CurseForge, class_id, loader_type};
use hyper_content::library::{installed, update};
use hyper_content::{ContentKind, SearchQuery, SearchSort, cfinstall, cfpack, install, mrpack};
use hyper_installer::Progress;
use hyper_instances::ContentDir;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::{AppState, CmdResult, OrCmd};
use crate::views::{
    CfFeaturedView, CfProjectView, ContentView, ExportView, InstanceView, PackImportView, PlanView,
    ProjectView, SearchView,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    pub text: String,
    pub kind: ContentKind,
    /// Mostra solo ciò che è compatibile con questa istanza.
    pub instance: Option<String>,
    /// Versione di Minecraft scelta nei filtri (senza istanza).
    #[serde(default)]
    pub game_version: Option<String>,
    /// Loader scelti nei filtri; vuoto = quelli dell'istanza (se c'è).
    #[serde(default)]
    pub loaders: Vec<String>,
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
    query.loaders = input.loaders;
    query.game_version = input.game_version.filter(|v| !v.is_empty());
    if let Some(name) = input.instance {
        let config = state.instances().load(&name).cmd()?.config;
        if query.loaders.is_empty() {
            let loader = config.loader.as_ref().map(|l| l.kind);
            if let Ok(loaders) = input.kind.loaders(loader) {
                query.loaders = loaders.iter().map(|l| (*l).to_owned()).collect();
            }
        }
        query.game_version = Some(config.version);
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
    let cf = state.curseforge.as_ref();
    match installed(&state.modrinth, cf, &target, dir, check_updates).await {
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
        state.curseforge.as_ref(),
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

/// Chiede un modpack (`.mrpack` di Modrinth o `.zip` di CurseForge) e lo
/// importa; `None` se l'utente annulla.
#[tauri::command]
pub async fn pack_import(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<PackImportView>> {
    let Some(file) = app
        .dialog()
        .file()
        .set_title("Importa un modpack")
        .add_filter("Modpack", &["mrpack", "zip"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().cmd()?;
    let report = progress_emitter(&app, "");
    let import = cfpack::import_any(
        state.curseforge.as_ref(),
        &state.instances(),
        state.installer().downloader(),
        &path,
        Some(&report),
    )
    .await
    .cmd()?;
    Ok(Some(PackImportView::from(import)))
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

// ---------- CurseForge ----------

fn curseforge(state: &AppState) -> CmdResult<&CurseForge> {
    state
        .curseforge
        .as_ref()
        .ok_or_else(|| "CurseForge non è disponibile in questa versione.".to_owned())
}

/// Progetti in evidenza, popolari e aggiornati di recente di un tipo. Con
/// un'istanza restano solo quelli che hanno un file per la sua versione
/// (e, per le mod, per il suo loader).
#[tauri::command]
pub async fn cf_featured(
    state: State<'_, AppState>,
    kind: ContentKind,
    instance: Option<String>,
) -> CmdResult<CfFeaturedView> {
    let lists = curseforge(&state)?.featured().await.cmd()?;
    let target = match instance {
        Some(name) => Some(state.instances().load(&name).cmd()?.config),
        None => None,
    };
    let class = class_id(kind);
    let fits = |m: &&CfMod| {
        m.class_id == Some(class)
            && target.as_ref().is_none_or(|c| {
                let loader = match kind {
                    ContentKind::Mod => c.loader.as_ref().map(|l| loader_type(l.kind)),
                    _ => None,
                };
                m.latest_file(&c.version, loader, false).is_some()
            })
    };
    let pick = |list: &[CfMod]| list.iter().filter(fits).map(CfProjectView::from).collect();
    Ok(CfFeaturedView {
        featured: pick(&lists.featured),
        popular: pick(&lists.popular),
        recent: pick(&lists.recently_updated),
    })
}

/// Un progetto per Project ID (il numero mostrato su curseforge.com).
#[tauri::command]
pub async fn cf_project(state: State<'_, AppState>, id: u32) -> CmdResult<CfProjectView> {
    let project = curseforge(&state)?.project(id).await.cmd()?;
    Ok(CfProjectView::from(&project))
}

/// Installa un progetto CurseForge e le sue dipendenze; restituisce quanti
/// file.
#[tauri::command]
pub async fn cf_install(
    app: AppHandle,
    state: State<'_, AppState>,
    instance: String,
    id: u32,
    world: Option<String>,
) -> CmdResult<usize> {
    let cf = curseforge(&state)?;
    let target = state.instances().load(&instance).cmd()?;
    let plan = cfinstall::plan(cf, &target, id, world.as_deref())
        .await
        .cmd()?;
    let report = progress_emitter(&app, &instance);
    plan.execute(&target, state.installer().downloader(), Some(&report))
        .await
        .cmd()?;
    Ok(plan.files.len())
}

/// Installa l'ultima versione di un modpack CurseForge come nuova istanza.
#[tauri::command]
pub async fn cf_pack_install(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u32,
) -> CmdResult<PackImportView> {
    let cf = curseforge(&state)?;
    let report = progress_emitter(&app, "");
    let import = cfpack::install_from_curseforge(
        cf,
        &state.instances(),
        state.installer().downloader(),
        &state.paths.cache_dir(),
        id,
        Some(&report),
    )
    .await
    .cmd()?;
    Ok(PackImportView::from(import))
}
