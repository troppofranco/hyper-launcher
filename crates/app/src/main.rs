//! Hyper Launcher: applicazione grafica. La finestra mostra l'interfaccia
//! Svelte (`ui/`); i comandi richiamano i crate del core.

// In release niente finestra di console accanto all'app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appearance;
mod backups;
mod commands;
mod content;
mod custom;
mod launch;
mod servers;
mod state;
mod views;

use state::AppState;
use tauri::Manager;

fn main() {
    // Modalità host dei server (avviata dal launcher o all'accesso a Windows):
    // nessuna finestra, solo i server in background.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == servers::HOST_ARG) {
        let autostart = args.iter().any(|a| a == "--autostart");
        if let Err(e) = servers::run_host(autostart) {
            eprintln!("server host: {e:#}");
            std::process::exit(1);
        }
        return;
    }

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
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        // Effetto trasparente della finestra (Mica/Acrylic) scelto nelle Impostazioni.
        .setup(|app| {
            let appearance = app.state::<AppState>().config().appearance;
            appearance::apply_window_effect(app.handle(), &appearance);
            // Tornando da "riduci a icona" Windows a volte perde l'effetto
            // trasparente: lo si riapplica una volta, al ripristino.
            if let Some(window) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                let minimized = std::sync::atomic::AtomicBool::new(false);
                let watched = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Resized(_) = event {
                        let now = watched.is_minimized().unwrap_or(false);
                        let was = minimized.swap(now, std::sync::atomic::Ordering::Relaxed);
                        if was && !now {
                            appearance::refresh_window_effect(&handle);
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            appearance::background,
            appearance::set_background,
            appearance::storage_usage,
            appearance::clear_download_cache,
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
            content::content_search,
            content::content_project,
            content::content_plan,
            content::content_install,
            content::content_installed,
            content::set_content_enabled,
            content::delete_content,
            content::content_update,
            content::pack_install,
            content::pack_import,
            content::cf_featured,
            content::cf_project,
            content::cf_install,
            content::cf_pack_install,
            content::pack_export,
            commands::list_worlds,
            commands::recent_sessions,
            custom::set_instance_icon,
            custom::list_skins,
            custom::default_skins,
            custom::add_skin,
            custom::remove_skin,
            custom::select_skin,
            custom::list_servers,
            custom::ping_server,
            commands::open_folder,
            commands::open_url,
            commands::news,
            commands::check_update,
            commands::install_update,
            launch::launch_instance,
            servers::server_list,
            servers::server_versions,
            servers::server_create,
            servers::server_pick_modpack,
            servers::server_create_modpack,
            servers::server_play,
            servers::server_crash,
            servers::server_delete,
            servers::server_start,
            servers::server_stop,
            servers::server_restart,
            servers::server_command,
            servers::server_console,
            servers::server_stats,
            servers::server_update,
            servers::server_set_icon,
            servers::server_set_schedule,
            backups::server_backups,
            backups::server_backup,
            backups::server_backup_restore,
            backups::server_backup_delete,
            backups::world_backups,
            backups::world_backup,
            backups::world_backup_restore,
            backups::world_backup_delete,
            servers::playit_status,
            servers::playit_connect,
            servers::playit_disconnect,
            servers::server_playit_enable,
            servers::server_playit_disable,
            servers::server_players,
            servers::server_properties,
            servers::server_save_properties,
            servers::server_files,
            servers::server_read_file,
            servers::server_write_file,
            servers::server_delete_file,
            servers::server_plugins,
            servers::server_plugin_install,
            servers::server_plugin_set_enabled,
            servers::server_plugin_delete,
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(|handle, event| {
            // Chiudendo il launcher si fermano i server senza "resta acceso".
            if let tauri::RunEvent::Exit = event {
                servers::on_exit(&handle.state::<AppState>());
            }
        }),
        Err(e) => {
            tracing::error!("cannot start the window: {e}");
            std::process::exit(1);
        }
    }
}
