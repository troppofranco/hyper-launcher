//! Hyper Launcher: applicazione grafica. La finestra mostra l'interfaccia
//! Svelte (`ui/`); i comandi richiamano i crate del core.

// In release niente finestra di console accanto all'app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod launch;
mod state;
mod views;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    let state = match AppState::load() {
        Ok(state) => state,
        Err(e) => {
            tracing::error!("cannot load launcher data: {e:#}");
            std::process::exit(1);
        }
    };

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::list_versions,
            commands::loader_versions,
            commands::list_accounts,
            commands::add_offline_account,
            commands::add_microsoft_account,
            commands::set_default_account,
            commands::remove_account,
            commands::get_settings,
            commands::save_settings,
            commands::list_mods,
            commands::set_mod_enabled,
            commands::delete_mod,
            commands::list_worlds,
            commands::open_folder,
            commands::open_url,
            commands::news,
            commands::check_update,
            commands::install_update,
            launch::launch_instance,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("cannot start the window: {e}");
        std::process::exit(1);
    }
}
