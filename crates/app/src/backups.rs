//! Comandi dei backup: mondi dei server (a server acceso li fa l'host) e
//! mondi delle istanze (solo a gioco chiuso). La logica sta in
//! `hyper_installer::backups` e `hyper_server::backup`.

use std::path::PathBuf;

use hyper_core::safe_component;
use hyper_installer::backups::{self, BackupInfo};
use hyper_server::{HostClient, ServerManager, backup};
use tauri::State;

use crate::state::{AppState, CmdResult, OrCmd};

fn manager(state: &AppState) -> ServerManager {
    ServerManager::new(state.paths.clone())
}

async fn server_running(state: &AppState, name: &str) -> bool {
    HostClient::new(&state.paths)
        .status()
        .await
        .iter()
        .any(|r| r.name == name)
}

// ---------- Server ----------

#[tauri::command]
pub fn server_backups(state: State<'_, AppState>, name: String) -> CmdResult<Vec<BackupInfo>> {
    backup::list(&state.paths, &name).cmd()
}

/// Backup ora: a server acceso lo fa l'host (sospende il salvataggio).
#[tauri::command]
pub async fn server_backup(state: State<'_, AppState>, name: String) -> CmdResult<BackupInfo> {
    if server_running(&state, &name).await {
        return HostClient::new(&state.paths)
            .backup(&name, false)
            .await
            .cmd();
    }
    let (manager, paths) = (manager(&state), state.paths.clone());
    tauri::async_runtime::spawn_blocking(move || backup::create_now(&manager, &paths, &name, false))
        .await
        .cmd()?
        .cmd()
}

#[tauri::command]
pub async fn server_backup_restore(
    state: State<'_, AppState>,
    name: String,
    file: String,
) -> CmdResult<Vec<String>> {
    if server_running(&state, &name).await {
        return Err("Ferma il server prima di ripristinare un backup.".into());
    }
    let (manager, paths) = (manager(&state), state.paths.clone());
    tauri::async_runtime::spawn_blocking(move || backup::restore(&manager, &paths, &name, &file))
        .await
        .cmd()?
        .cmd()
}

#[tauri::command]
pub fn server_backup_delete(
    state: State<'_, AppState>,
    name: String,
    file: String,
) -> CmdResult<()> {
    backup::delete(&state.paths, &name, &file).cmd()
}

// ---------- Mondi delle istanze ----------

/// Cartella dei backup di un mondo e cartella `saves` dell'istanza.
fn world_dirs(state: &AppState, name: &str, world: &str) -> CmdResult<(PathBuf, PathBuf)> {
    let instance = state.instances().load(name).cmd()?;
    let world = safe_component(world).cmd()?;
    let dir = state
        .paths
        .root()
        .join("backups")
        .join("instances")
        .join(safe_component(name).cmd()?)
        .join(world);
    Ok((dir, instance.saves_dir()))
}

/// A gioco aperto il mondo cambia mentre lo si copia: niente backup né
/// ripristini finché l'istanza è in uso.
fn ensure_closed(state: &AppState, name: &str) -> CmdResult<()> {
    if state.is_busy(name) {
        return Err("Chiudi il gioco prima di fare o ripristinare un backup del mondo.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn world_backups(
    state: State<'_, AppState>,
    name: String,
    world: String,
) -> CmdResult<Vec<BackupInfo>> {
    let (dir, _) = world_dirs(&state, &name, &world)?;
    Ok(backups::list(&dir))
}

#[tauri::command]
pub async fn world_backup(
    state: State<'_, AppState>,
    name: String,
    world: String,
) -> CmdResult<BackupInfo> {
    ensure_closed(&state, &name)?;
    let (dir, saves) = world_dirs(&state, &name, &world)?;
    if !saves.join(&world).join("level.dat").is_file() {
        return Err("Mondo non trovato.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let info = backups::create(&dir, &[(saves.join(&world), world.clone())], false).cmd()?;
        backups::prune(&dir, 10).cmd()?;
        Ok(info)
    })
    .await
    .cmd()?
}

#[tauri::command]
pub async fn world_backup_restore(
    state: State<'_, AppState>,
    name: String,
    world: String,
    file: String,
) -> CmdResult<()> {
    ensure_closed(&state, &name)?;
    let (dir, saves) = world_dirs(&state, &name, &world)?;
    tauri::async_runtime::spawn_blocking(move || {
        backups::restore(&dir, &file, &saves, &[world])
            .cmd()
            .map(drop)
    })
    .await
    .cmd()?
}

#[tauri::command]
pub fn world_backup_delete(
    state: State<'_, AppState>,
    name: String,
    world: String,
    file: String,
) -> CmdResult<()> {
    let (dir, _) = world_dirs(&state, &name, &world)?;
    backups::delete(&dir, &file).cmd()
}
