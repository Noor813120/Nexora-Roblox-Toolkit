//! Game Launcher commands.

use tauri::async_runtime;

use crate::error::AppError;
use crate::launcher::process::{self, RunningRobloxClients};

/// Takes one read-only snapshot of running Roblox Player processes.
///
/// This command does not open, modify, terminate, or launch any process.
#[tauri::command]
pub async fn get_running_roblox_clients() -> Result<RunningRobloxClients, AppError> {
    async_runtime::spawn_blocking(process::detect)
        .await
        .map_err(|error| {
            eprintln!("[launcher] process detection task failed: {error}");
            AppError::Internal
        })?
        .map_err(|error| {
            eprintln!("[launcher] process detection failed: {error}");
            AppError::Internal
        })
}
