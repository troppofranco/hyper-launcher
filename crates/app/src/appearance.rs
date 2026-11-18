//! Aspetto della finestra: effetto trasparente di Windows (Mica/Acrylic),
//! immagine di sfondo personalizzata e spazio occupato su disco.

use std::path::{Path, PathBuf};

use hyper_core::{AppearanceConfig, fs};
use hyper_instances::image_mime;
use serde::Serialize;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Manager, State};

use crate::custom::{data_url, decode};
use crate::state::{AppState, CmdResult, OrCmd};

/// Limite dell'immagine di sfondo: l'interfaccia la riduce prima (2560 px).
const MAX_BACKGROUND_BYTES: usize = 8 * 1024 * 1024;

/// Applica (o toglie) l'effetto trasparente alla finestra principale. Se
/// Mica non c'è (Windows 10) si ripiega su Acrylic. Restituisce `true` se lo
/// sfondo trasparente è attivo: altrimenti l'interfaccia usa la tinta unita
/// (una finestra trasparente senza effetto mostrerebbe il desktop nitido).
pub fn apply_window_effect(app: &AppHandle, appearance: &AppearanceConfig) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    let wanted = appearance.background == "transparent";
    let result = if wanted {
        let effect = |e: Effect| window.set_effects(EffectsBuilder::new().effect(e).build());
        if appearance.effect == "acrylic" {
            effect(Effect::Acrylic)
        } else {
            effect(Effect::Mica).or_else(|_| effect(Effect::Acrylic))
        }
    } else {
        window.set_effects(None)
    };
    let ok = match result {
        Ok(()) => wanted,
        Err(e) => {
            tracing::warn!(error = %e, "cannot apply the window effect");
            false
        }
    };
    app.state::<AppState>().set_window_effect(ok);
    ok
}

fn background_file(state: &AppState) -> PathBuf {
    state.paths.root().join("appearance").join("background")
}

/// Immagine di sfondo come `data:` URL, se c'è.
#[tauri::command]
pub async fn background(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let path = background_file(&state);
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(path).ok())
        .await
        .cmd()?;
    Ok(bytes.and_then(|b| Some(data_url(image_mime(&b)?, &b))))
}

/// Salva (immagine in base64, già ridotta dall'interfaccia) o toglie
/// l'immagine di sfondo.
#[tauri::command]
pub fn set_background(state: State<'_, AppState>, image: Option<String>) -> CmdResult<()> {
    let path = background_file(&state);
    let Some(image) = image else {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    };
    let bytes = decode(&image)?;
    if bytes.len() > MAX_BACKGROUND_BYTES || image_mime(&bytes).is_none() {
        return Err("L'immagine deve essere PNG, JPEG, GIF o WebP, al massimo 8 MB.".into());
    }
    fs::write_atomic(&path, &bytes).cmd()
}

/// Spazio occupato dalle cartelle del launcher, in byte.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageView {
    pub instances: u64,
    pub libraries: u64,
    pub assets: u64,
    pub versions: u64,
    pub runtimes: u64,
    pub cache: u64,
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            // I collegamenti non si seguono: niente doppi conteggi o cicli.
            Ok(t) if t.is_dir() => dir_size(&entry.path()),
            Ok(t) if t.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

#[tauri::command]
pub async fn storage_usage(state: State<'_, AppState>) -> CmdResult<StorageView> {
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || StorageView {
        instances: dir_size(&paths.instances_dir()),
        libraries: dir_size(&paths.libraries_dir()),
        assets: dir_size(&paths.assets_dir()),
        versions: dir_size(&paths.versions_dir()),
        runtimes: dir_size(&paths.runtimes_dir()),
        cache: dir_size(&paths.cache_dir()),
    })
    .await
    .cmd()
}

/// Svuota la cache dei download (installer dei loader, modpack scaricati).
/// Librerie, asset e istanze restano: il gioco non va riscaricato.
#[tauri::command]
pub async fn clear_download_cache(state: State<'_, AppState>) -> CmdResult<()> {
    if state.any_busy() {
        return Err("Aspetta che finiscano installazioni e partite in corso.".into());
    }
    let cache = state.paths.cache_dir();
    tauri::async_runtime::spawn_blocking(move || fs::remove_dir_all_if_exists(&cache))
        .await
        .cmd()?
        .cmd()
}
