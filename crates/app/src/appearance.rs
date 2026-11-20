//! Aspetto della finestra: effetto trasparente di Windows (Mica/Acrylic),
//! immagine di sfondo personalizzata e spazio occupato su disco.

use std::path::{Path, PathBuf};

use hyper_core::{AppearanceConfig, fs};
use hyper_instances::image_mime;
use serde::Serialize;
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{AppHandle, Manager, State};

use crate::custom::{data_url, decode};
use crate::state::{AppState, CmdResult, OrCmd};

/// Limite dell'immagine di sfondo: l'interfaccia la riduce prima (2560 px).
const MAX_BACKGROUND_BYTES: usize = 8 * 1024 * 1024;

/// Applica aspetto e scala alla finestra principale: zoom nativo della
/// WebView (come Ctrl +/−, ridimensiona tutta la pagina in modo coerente) ed
/// effetto trasparente. L'intensità della sfocatura è uno degli effetti di
/// Windows: Blur (leggera), Acrylic (media), Mica (forte; su Windows 10 si
/// ripiega su Acrylic). Restituisce `true` se lo sfondo trasparente è attivo:
/// altrimenti l'interfaccia usa la tinta unita (una finestra trasparente senza
/// effetto mostrerebbe il desktop nitido).
pub fn apply_window_effect(app: &AppHandle, appearance: &AppearanceConfig) -> bool {
    apply(app, appearance, false)
}

/// Ultimi valori applicati alla finestra (scala, sfondo, effetto). Le
/// impostazioni si salvano a ogni modifica: riapplicare ogni volta l'effetto
/// di Windows fa lampeggiare la finestra, quindi lo si fa solo se cambia.
static APPLIED: std::sync::Mutex<Option<(u32, bool, String)>> = std::sync::Mutex::new(None);

/// Riapplica l'effetto anche se uguale (es. quando la finestra torna in
/// primo piano o dopo essere stata ridotta a icona: Windows a volte lo perde).
pub fn refresh_window_effect(app: &AppHandle) {
    let appearance = app.state::<AppState>().config().appearance;
    if appearance.background == "transparent" {
        apply(app, &appearance, true);
    }
}

fn apply(app: &AppHandle, appearance: &AppearanceConfig, force: bool) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    let wanted = appearance.background == "transparent";
    let current = (appearance.ui_scale, wanted, appearance.effect.clone());
    let previous = APPLIED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(current.clone());
    if previous.as_ref().map(|p| p.0) != Some(current.0)
        && let Err(e) = window.set_zoom(f64::from(appearance.ui_scale) / 100.0)
    {
        tracing::warn!(error = %e, "cannot apply the interface scale");
    }
    let state = app.state::<AppState>();
    if !force
        && previous
            .as_ref()
            .is_some_and(|p| p.1 == current.1 && p.2 == current.2)
    {
        return state.window_effect();
    }
    let result = if wanted {
        // Sempre attivo: con lo stato predefinito Windows spegne Mica e
        // Acrylic (tinta piatta) quando la finestra perde il focus.
        let effect = |e: Effect| {
            window.set_effects(
                EffectsBuilder::new()
                    .effect(e)
                    .state(EffectState::Active)
                    .build(),
            )
        };
        match appearance.effect.as_str() {
            "blur" => effect(Effect::Blur).or_else(|_| effect(Effect::Acrylic)),
            "acrylic" => effect(Effect::Acrylic),
            _ => effect(Effect::Mica).or_else(|_| effect(Effect::Acrylic)),
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
