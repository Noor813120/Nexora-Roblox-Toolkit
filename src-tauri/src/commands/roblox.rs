//! Roblox commands. Milestone 1 has one: a read-only check for an installed Roblox player.
//!
//! The result goes to the screen only. It is deliberately not written to the activity log or to
//! the database, and the command takes no database state at all.

use crate::error::AppError;
use crate::launcher::roblox::{RobloxDetection, detect};
use crate::platform::roblox::SystemEnvironment;

/// Looks for Roblox on this computer and reports what was found and what could not be checked.
///
/// Read-only: it lists a folder, checks that files exist and reads one registry value. It never
/// runs the registry command, starts a process, or touches the network. The file work happens on a
/// blocking thread so the window stays responsive.
#[tauri::command]
pub async fn detect_roblox() -> Result<RobloxDetection, AppError> {
    tauri::async_runtime::spawn_blocking(run_detection)
        .await
        .map_err(|e| {
            eprintln!("[roblox] detection task failed: {e}");
            AppError::Internal
        })
}

/// The real detection, separated from the async wrapper so it can be tested directly.
fn run_detection() -> RobloxDetection {
    detect(&SystemEnvironment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::roblox::DetectionStatus;

    /// Not Windows: the honest answer is "unsupported", never "not installed".
    #[cfg(not(windows))]
    #[test]
    fn reports_unsupported_off_windows() {
        let d = run_detection();
        assert_eq!(d.status, DetectionStatus::Unsupported);
        assert!(d.installs.is_empty());
    }

    /// On Windows any status except unsupported is legitimate; the machine decides which.
    #[cfg(windows)]
    #[test]
    fn runs_on_windows() {
        assert_ne!(run_detection().status, DetectionStatus::Unsupported);
    }

    #[test]
    fn the_command_returns_the_same_kind_of_answer() {
        let via_command = tauri::async_runtime::block_on(detect_roblox()).expect("no task failure");
        assert_eq!(via_command.status, run_detection().status);
    }
}
