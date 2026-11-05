//! IPC commands: the only way the frontend reaches Rust.
//!
//! Rules for every command added here: typed request and response structs (exported to TypeScript
//! with ts-rs), validation before any work, `Result<_, AppError>`, and no process handles or other
//! internals in the response.

use serde::Serialize;
use ts_rs::TS;

use crate::platform;

/// Static facts about this build, shown on the Phase 1 screen.
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
}
