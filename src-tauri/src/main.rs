// Hides the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Nexora backend.
//!
//! Rust owns every Windows process operation, task and sensitive action; the React frontend only
//! presents state and sends typed commands. See `docs/ARCHITECTURE.md`.

mod automation;
mod commands;
mod error;
mod launcher;
mod monitoring;
mod platform;
mod scripts;
mod security;
mod storage;

use tauri::Manager;

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // A database problem must not stop the window from opening: the UI explains it.
            let state = match commands::database::resolve_db_path(app.handle()) {
                Ok(path) => commands::database::init_state(&path, storage::clock::now_ms()),
                Err(message) => {
                    eprintln!("[storage] {message}");
                    commands::database::unavailable_state(message)
                }
            };
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::system_info,
            commands::database::database_status,
            commands::database::get_setting,
            commands::database::list_settings,
            commands::database::set_setting,
            commands::database::delete_setting,
            commands::database::list_profiles,
            commands::database::create_profile,
            commands::database::update_profile,
            commands::database::delete_profile,
            commands::database::list_automation_profiles,
            commands::database::create_automation_profile,
            commands::database::update_automation_profile,
            commands::database::delete_automation_profile,
            commands::database::list_activity_logs,
            commands::database::create_activity_log,
            commands::database::clear_old_activity_logs,
            commands::database::backup_database,
            commands::database::restore_database,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Nexora");

    app.run(|handle, event| {
        // Flush the write-ahead log so the database file on disk is complete when the app closes.
        if let tauri::RunEvent::Exit = event
            && let Some(state) = handle.try_state::<commands::database::DbState>()
        {
            commands::database::checkpoint_on_exit(&state);
        }
    });
}
