// Hides the extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! RoALT Advanced backend.
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("error while running RoALT Advanced");
}
