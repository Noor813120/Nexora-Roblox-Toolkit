//! Read-only discovery of running Roblox Player processes.
//!
//! A matching process is evidence that RobloxPlayerBeta.exe exists in the process
//! snapshot. It does not prove that the process is responsive, logged in, or in a game.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProcessScanStatus {
    #[allow(dead_code)] // Constructed only by the Windows implementation.
    Available,
    #[allow(dead_code)] // Constructed by the non-Windows implementation.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunningRobloxClient {
    pub process_id: u32,
    pub executable_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunningRobloxClients {
    pub status: ProcessScanStatus,
    pub clients: Vec<RunningRobloxClient>,
}

#[cfg(any(windows, test))]
const ROBLOX_PLAYER_EXE: &str = "RobloxPlayerBeta.exe";

#[cfg(any(windows, test))]
fn is_roblox_player_executable(name: &str) -> bool {
    name.eq_ignore_ascii_case(ROBLOX_PLAYER_EXE)
}

#[cfg(any(windows, test))]
fn collect_matching_clients(
    entries: impl IntoIterator<Item = (u32, String)>,
) -> Vec<RunningRobloxClient> {
    let mut clients: Vec<_> = entries
        .into_iter()
        .filter_map(|(process_id, executable_name)| {
            is_roblox_player_executable(&executable_name).then_some(RunningRobloxClient {
                process_id,
                executable_name,
            })
        })
        .collect();

    clients.sort_by_key(|client| client.process_id);
    clients
}

/// Takes one read-only snapshot of the running process list.
#[cfg(windows)]
pub fn detect() -> Result<RunningRobloxClients, String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    struct SnapshotHandle(HANDLE);

    impl Drop for SnapshotHandle {
        fn drop(&mut self) {
            // Always release the snapshot, including when enumeration returns an error.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|error| format!("Could not create process snapshot: {error}"))?;

    let snapshot = SnapshotHandle(snapshot);

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    unsafe { Process32FirstW(snapshot.0, &mut entry) }
        .map_err(|error| format!("Could not read the first process entry: {error}"))?;

    let mut entries = Vec::new();

    loop {
        let name_length = entry
            .szExeFile
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(entry.szExeFile.len());

        let executable_name = String::from_utf16_lossy(&entry.szExeFile[..name_length]);

        entries.push((entry.th32ProcessID, executable_name));

        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        match unsafe { Process32NextW(snapshot.0, &mut entry) } {
            Ok(()) => {}
            Err(error) if error.code().0 == 0x8007_0012_u32 as i32 => {
                // HRESULT_FROM_WIN32(ERROR_NO_MORE_FILES): normal end of snapshot.
                break;
            }
            Err(error) => {
                return Err(format!("Could not continue process enumeration: {error}"));
            }
        }
    }

    Ok(RunningRobloxClients {
        status: ProcessScanStatus::Available,
        clients: collect_matching_clients(entries),
    })
}

/// Process discovery is currently implemented only on Windows.
#[cfg(not(windows))]
pub fn detect() -> Result<RunningRobloxClients, String> {
    Ok(RunningRobloxClients {
        status: ProcessScanStatus::Unsupported,
        clients: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_exact_player_executable_case_insensitively() {
        assert!(is_roblox_player_executable("RobloxPlayerBeta.exe"));
        assert!(is_roblox_player_executable("robloxplayerbeta.exe"));
        assert!(is_roblox_player_executable("ROBLOXPLAYERBETA.EXE"));

        assert!(!is_roblox_player_executable("RobloxStudioBeta.exe"));
        assert!(!is_roblox_player_executable("RobloxPlayerLauncher.exe"));
        assert!(!is_roblox_player_executable("Other.exe"));
        assert!(!is_roblox_player_executable("RobloxPlayerBeta.exe.old"));
    }

    #[test]
    fn collects_only_matching_processes() {
        let clients = collect_matching_clients([
            (100, "explorer.exe".into()),
            (200, "RobloxPlayerBeta.exe".into()),
            (300, "RobloxStudioBeta.exe".into()),
            (400, "robloxplayerbeta.EXE".into()),
        ]);

        assert_eq!(
            clients,
            vec![
                RunningRobloxClient {
                    process_id: 200,
                    executable_name: "RobloxPlayerBeta.exe".into(),
                },
                RunningRobloxClient {
                    process_id: 400,
                    executable_name: "robloxplayerbeta.EXE".into(),
                },
            ]
        );
    }

    #[test]
    fn sorts_matching_processes_by_pid() {
        let clients = collect_matching_clients([
            (900, "RobloxPlayerBeta.exe".into()),
            (100, "RobloxPlayerBeta.exe".into()),
            (500, "RobloxPlayerBeta.exe".into()),
        ]);

        assert_eq!(
            clients
                .iter()
                .map(|client| client.process_id)
                .collect::<Vec<_>>(),
            vec![100, 500, 900]
        );
    }

    #[test]
    fn returns_an_empty_list_when_no_player_process_matches() {
        let clients = collect_matching_clients([
            (100, "explorer.exe".into()),
            (200, "RobloxStudioBeta.exe".into()),
        ]);

        assert!(clients.is_empty());
    }

    #[cfg(not(windows))]
    #[test]
    fn reports_unsupported_off_windows() {
        let result = detect().expect("unsupported detection is a valid response");

        assert_eq!(result.status, ProcessScanStatus::Unsupported);
        assert!(result.clients.is_empty());
    }
}
