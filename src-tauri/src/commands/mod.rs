//! IPC commands: the only way the frontend reaches Rust.
//!
//! Rules for every command added here: typed request and response structs (exported to TypeScript
//! with ts-rs), validation before any work, `Result<_, AppError>`, and no process handles or other
//! internals in the response.

use serde::Serialize;
use ts_rs::TS;

use crate::platform;

/// Static facts about this build, shown on the Dashboard and in Settings.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
}

fn build_info() -> AppInfo {
    AppInfo {
        name: "RoALT Advanced".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: platform::os().into(),
        arch: platform::arch().into(),
    }
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    build_info()
}

/// Physical memory at one moment. Present only where the OS call is implemented (Windows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

/// Basic facts about this computer. A field is `null` when it cannot be read reliably; the
/// frontend must show "unavailable", never a made-up number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub logical_cores: Option<u32>,
    pub memory: Option<MemoryInfo>,
}

fn system_info_from(cores: Option<u32>, memory: Option<platform::MemoryStatus>) -> SystemInfo {
    SystemInfo {
        logical_cores: cores,
        memory: memory.map(|m| MemoryInfo {
            total_bytes: m.total,
            used_bytes: m.used(),
            available_bytes: m.available,
        }),
    }
}

/// A snapshot, read once per call. The frontend asks on load and on an explicit refresh; it does
/// not poll.
#[tauri::command]
pub fn system_info() -> SystemInfo {
    system_info_from(platform::logical_cores(), platform::memory_status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_matches_the_crate_version() {
        let info = build_info();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.name, "RoALT Advanced");
        assert!(!info.platform.is_empty() && !info.arch.is_empty());
    }

    #[test]
    fn app_info_serializes_in_camel_case() {
        let v = serde_json::to_value(build_info()).unwrap();
        for key in ["name", "version", "platform", "arch"] {
            assert!(v.get(key).is_some(), "missing field {key}");
        }
        assert_eq!(v.as_object().unwrap().len(), 4);
    }

    #[test]
    fn system_info_maps_memory_and_computes_used() {
        let info = system_info_from(
            Some(8),
            Some(platform::MemoryStatus {
                total: 16_000,
                available: 6_000,
            }),
        );
        assert_eq!(info.logical_cores, Some(8));
        let m = info.memory.expect("memory present");
        assert_eq!(
            (m.total_bytes, m.used_bytes, m.available_bytes),
            (16_000, 10_000, 6_000)
        );
    }

    #[test]
    fn system_info_serializes_unavailable_values_as_null() {
        let v = serde_json::to_value(system_info_from(None, None)).unwrap();
        assert!(v["logicalCores"].is_null());
        assert!(v["memory"].is_null());
    }

    #[test]
    fn system_info_serializes_memory_in_camel_case() {
        let info = system_info_from(
            Some(4),
            Some(platform::MemoryStatus {
                total: 10,
                available: 4,
            }),
        );
        let v = serde_json::to_value(info).unwrap();
        assert_eq!(v["logicalCores"], 4);
        assert_eq!(v["memory"]["totalBytes"], 10);
        assert_eq!(v["memory"]["usedBytes"], 6);
        assert_eq!(v["memory"]["availableBytes"], 4);
    }

    #[test]
    fn the_real_command_never_panics() {
        // Windows fills memory in; elsewhere it is null. Either way the call must succeed.
        let info = system_info();
        assert!(info.logical_cores.is_some_and(|n| n >= 1));
    }
}
