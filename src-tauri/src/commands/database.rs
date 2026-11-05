//! IPC commands for the database. Thin on purpose: validate, hand the work to `storage` on a
//! blocking thread, convert errors. No command accepts SQL, and the frontend never supplies a file
//! path for backup/restore: Rust opens the native file dialog itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use ts_rs::TS;

use crate::error::AppError;
use crate::storage::Database;
use crate::storage::activity_logs::{
    self, ActivityLog, ActivityLogPage, ActivityLogQuery, NewActivityLog, Severity,
};
use crate::storage::automation_profiles::{
    self as automation, AutomationProfile, AutomationProfileInput,
};
use crate::storage::clock::{file_stamp, now_ms};
use crate::storage::database::{DATA_DIR_NAME, DB_FILE_NAME};
use crate::storage::errors::{Result as StorageResult, StorageError};
use crate::storage::profiles::{self, Profile, ProfileInput};
use crate::storage::settings::{self, SettingEntry};

// ---------------------------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------------------------

/// What the app learned while opening the database. `db` is `None` when opening failed; every
/// command then answers `unavailable` and the UI shows `error`.
pub struct DbState {
    db: Option<Arc<Database>>,
    recovered_file: Option<String>,
    error: Option<String>,
    location: Option<PathBuf>,
}

impl DbState {
    pub fn database(&self) -> Option<&Arc<Database>> {
        self.db.as_ref()
    }
}

/// Where the data folder is: the OS local-data directory (via Tauri) plus `RoALT-Advanced`.
/// On Windows that is `%LOCALAPPDATA%\RoALT-Advanced`.
pub fn resolve_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .local_data_dir()
        .map(|d| d.join(DATA_DIR_NAME).join(DB_FILE_NAME))
        .map_err(|e| format!("the system data folder could not be determined: {e}"))
}

/// A short, safe explanation for the user. The technical detail is in the log.
fn describe_open_failure(e: &StorageError) -> String {
    match e {
        StorageError::SchemaTooNew { .. } => "This database was created by a newer version of \
            Nexora. Update the app to use it. Nothing was changed."
            .into(),
        StorageError::MigrationFailed { .. } | StorageError::MigrationMismatch { .. } => {
            "The database could not be upgraded. Your data was left as it was, and a copy was \
             kept in the app's data folder."
                .into()
        }
        StorageError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => {
            "The app does not have permission to use its data folder.".into()
        }
        StorageError::Io(_) => {
            "The app's data folder could not be used (is the disk full or the folder missing?)."
                .into()
        }
        _ => "The database could not be opened.".into(),
    }
}

fn log_event(
    conn: &mut Connection,
    kind: &str,
    severity: Severity,
    message: &str,
    metadata: Option<serde_json::Value>,
    now: i64,
) {
    let entry = NewActivityLog {
        event_type: kind.into(),
        severity,
        message: message.into(),
        metadata_json: metadata.map(|m| m.to_string()),
    };
    // Logging must never break the operation being logged, but a failure is not hidden either.
    if let Err(e) = activity_logs::create(conn, &entry, now) {
        eprintln!("[storage] could not write activity log '{kind}': {e}");
    }
}

/// State for the case where not even the data folder could be located.
pub fn unavailable_state(message: String) -> DbState {
    DbState {
        db: None,
        recovered_file: None,
        error: Some(message),
        location: None,
    }
}

/// Opens the database and records the real startup events. Never panics; failure becomes state.
pub fn init_state(path: &Path, now: i64) -> DbState {
    match Database::open(path, now) {
        Ok((db, report)) => {
            let recovered_file = report
                .recovered_from
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned());
            let version = env!("CARGO_PKG_VERSION");
            let result = db.with(|c| {
                if let Some(file) = &recovered_file {
                    log_event(
                        c,
                        "database.recovered",
                        Severity::Error,
                        "The previous database file was damaged. It was moved aside and a new database was created.",
                        Some(serde_json::json!({ "movedTo": format!("quarantine/{file}") })),
                        now,
                    );
                }
                if !report.migrated.is_empty() {
                    log_event(
                        c,
                        "database.migrated",
                        Severity::Info,
                        &format!("Database schema updated to version {}.", report.schema_version),
                        Some(serde_json::json!({ "applied": report.migrated })),
                        now,
                    );
                }
                let removed = activity_logs::prune_by_retention(c, now)?;
                if removed > 0 {
                    log_event(
                        c,
                        "activity.pruned",
                        Severity::Debug,
                        &format!("Removed {removed} activity entries past the retention period."),
                        None,
                        now,
                    );
                }
                log_event(
                    c,
                    "app.started",
                    Severity::Info,
                    &format!("Nexora {version} started."),
                    None,
                    now,
                );
                Ok(())
            });
            if let Err(e) = result {
                eprintln!("[storage] startup bookkeeping failed: {e}");
            }
            DbState {
                db: Some(Arc::new(db)),
                recovered_file,
                error: None,
                location: Some(path.to_owned()),
            }
        }
        Err(e) => {
            eprintln!("[storage] cannot open the database: {e}");
            DbState {
                db: None,
                recovered_file: None,
                error: Some(describe_open_failure(&e)),
                location: Some(path.to_owned()),
            }
        }
    }
}

/// Runs `f` on a blocking thread with the connection, so the async runtime is never held up by
/// disk I/O.
async fn run<T, F>(state: &State<'_, DbState>, f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> StorageResult<T> + Send + 'static,
{
    let db = state.database().cloned().ok_or(AppError::Unavailable)?;
    tauri::async_runtime::spawn_blocking(move || db.with(f))
        .await
        .map_err(|e| {
            eprintln!("[storage] blocking task failed: {e}");
            AppError::Internal
        })?
        .map_err(AppError::from)
}

// ---------------------------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DatabaseStatus {
    pub available: bool,
    /// Full path of the database file (shown in Settings so the user can find their data).
    pub location: Option<String>,
    pub schema_version: Option<u32>,
    pub journal_mode: Option<String>,
    /// Size of the main database file (the write-ahead log is not included).
    pub size_bytes: Option<u64>,
    /// Set when a damaged database was replaced at startup: the name of the preserved file.
    pub recovered_file: Option<String>,
    /// Why the database is unavailable, in words for the user.
    pub error: Option<String>,
}

/// A status field that could not be read becomes `None` (shown as "—"), and the reason is logged
/// instead of being dropped.
fn or_log<T, E: std::fmt::Display>(what: &str, result: Result<T, E>) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[storage] could not read {what} for the status: {e}");
            None
        }
    }
}

fn status_of(state: &DbState) -> DatabaseStatus {
    let location = state.location.as_ref().map(|p| p.display().to_string());
    match &state.db {
        Some(db) => DatabaseStatus {
            available: true,
            location,
            schema_version: or_log("schema version", db.schema_version()),
            journal_mode: or_log("journal mode", db.journal_mode()),
            size_bytes: or_log("file size", std::fs::metadata(db.path())).map(|m| m.len()),
            recovered_file: state.recovered_file.clone(),
            error: None,
        },
        None => DatabaseStatus {
            available: false,
            location,
            schema_version: None,
            journal_mode: None,
            size_bytes: None,
            recovered_file: None,
            error: state.error.clone(),
        },
    }
}

#[tauri::command]
pub async fn database_status(state: State<'_, DbState>) -> Result<DatabaseStatus, AppError> {
    // Reads only a few pragmas and the file size; fast enough to run inline.
    let db = state.database().cloned();
    let snapshot = DbState {
        db,
        recovered_file: state.recovered_file.clone(),
        error: state.error.clone(),
        location: state.location.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || status_of(&snapshot))
        .await
        .map_err(|_| AppError::Internal)
}

// ---------------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_setting(state: State<'_, DbState>, key: String) -> Result<SettingEntry, AppError> {
    run(&state, move |c| settings::entry(c, &key)).await
}

#[tauri::command]
pub async fn list_settings(state: State<'_, DbState>) -> Result<Vec<SettingEntry>, AppError> {
    run(&state, |c| settings::entries(c)).await
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, DbState>,
    key: String,
    value: String,
) -> Result<SettingEntry, AppError> {
    run(&state, move |c| settings::set(c, &key, &value, now_ms())).await
}

/// Resets a setting to its default (removes the saved value) and returns the effective entry.
#[tauri::command]
pub async fn delete_setting(
    state: State<'_, DbState>,
    key: String,
) -> Result<SettingEntry, AppError> {
    run(&state, move |c| settings::reset(c, &key)).await
}

// ---------------------------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn list_profiles(state: State<'_, DbState>) -> Result<Vec<Profile>, AppError> {
    run(&state, |c| profiles::list(c)).await
}

#[tauri::command]
pub async fn create_profile(
    state: State<'_, DbState>,
    input: ProfileInput,
) -> Result<Profile, AppError> {
    run(&state, move |c| profiles::create(c, &input, now_ms())).await
}

#[tauri::command]
pub async fn update_profile(
    state: State<'_, DbState>,
    id: i64,
    input: ProfileInput,
) -> Result<Profile, AppError> {
    run(&state, move |c| profiles::update(c, id, &input, now_ms())).await
}

#[tauri::command]
pub async fn delete_profile(state: State<'_, DbState>, id: i64) -> Result<(), AppError> {
    run(&state, move |c| profiles::delete(c, id)).await
}

// ---------------------------------------------------------------------------------------------
// Automation profiles (configuration only; nothing runs in Phase 3)
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn list_automation_profiles(
    state: State<'_, DbState>,
) -> Result<Vec<AutomationProfile>, AppError> {
    run(&state, |c| automation::list(c)).await
}

#[tauri::command]
pub async fn create_automation_profile(
    state: State<'_, DbState>,
    input: AutomationProfileInput,
) -> Result<AutomationProfile, AppError> {
    run(&state, move |c| automation::create(c, &input, now_ms())).await
}

#[tauri::command]
pub async fn update_automation_profile(
    state: State<'_, DbState>,
    id: i64,
    input: AutomationProfileInput,
) -> Result<AutomationProfile, AppError> {
    run(&state, move |c| automation::update(c, id, &input, now_ms())).await
}

#[tauri::command]
pub async fn delete_automation_profile(state: State<'_, DbState>, id: i64) -> Result<(), AppError> {
    run(&state, move |c| automation::delete(c, id)).await
}

// ---------------------------------------------------------------------------------------------
// Activity logs
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn list_activity_logs(
    state: State<'_, DbState>,
    query: ActivityLogQuery,
) -> Result<ActivityLogPage, AppError> {
    run(&state, move |c| activity_logs::list(c, &query)).await
}

#[tauri::command]
pub async fn create_activity_log(
    state: State<'_, DbState>,
    entry: NewActivityLog,
) -> Result<ActivityLog, AppError> {
    run(&state, move |c| activity_logs::create(c, &entry, now_ms())).await
}

/// Deletes entries older than `older_than_days` (1..=3650); returns how many were removed.
#[tauri::command]
pub async fn clear_old_activity_logs(
    state: State<'_, DbState>,
    older_than_days: u32,
) -> Result<u32, AppError> {
    run(&state, move |c| {
        let now = now_ms();
        let removed = activity_logs::clear_older_than_days(c, older_than_days, now)?;
        if removed > 0 {
            log_event(
                c,
                "activity.cleared",
                Severity::Info,
                &format!("Removed {removed} activity entries older than {older_than_days} days."),
                None,
                now,
            );
        }
        Ok(u32::try_from(removed).unwrap_or(u32::MAX))
    })
    .await
}

// ---------------------------------------------------------------------------------------------
// Backup and restore
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupOutcome {
    /// `false` when the user closed the dialog without choosing a file.
    pub saved: bool,
    pub file_name: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreOutcome {
    /// `false` when the user closed the dialog without choosing a file.
    pub restored: bool,
    pub schema_version: Option<u32>,
    pub migrated_versions: Vec<u32>,
    /// File name of the copy of the previous database, kept in the app's data folder.
    pub safety_copy: Option<String>,
}

fn file_name_of(p: &Path) -> Option<String> {
    p.file_name().map(|n| n.to_string_lossy().into_owned())
}

/// Backup after the user picked `dest`. `replace_existing` is true only when the chosen file
/// already existed, because the operating system's save dialog asks "replace it?" in that case.
fn do_backup(
    db: &Database,
    dest: &Path,
    replace_existing: bool,
    now: i64,
) -> StorageResult<BackupOutcome> {
    let info = db.backup_to(dest, replace_existing)?;
    let name = file_name_of(dest);
    let size = info.size_bytes;
    db.with(|c| {
        log_event(
            c,
            "database.backup_created",
            Severity::Info,
            &format!("Database backup saved ({} KB).", size.div_ceil(1024)),
            Some(serde_json::json!({ "fileName": name })),
            now,
        );
        Ok(())
    })?;
    Ok(BackupOutcome {
        saved: true,
        file_name: name,
        size_bytes: Some(size),
    })
}

fn do_restore(db: &Database, src: &Path, now: i64) -> StorageResult<RestoreOutcome> {
    let info = db.restore_from(src, now)?;
    db.with(|c| {
        log_event(
            c,
            "database.restored",
            Severity::Warning,
            "Database restored from a backup. The previous data was kept as a safety copy.",
            Some(serde_json::json!({
                "fileName": file_name_of(src),
                "safetyCopy": info.safety_copy,
            })),
            now,
        );
        Ok(())
    })?;
    Ok(RestoreOutcome {
        restored: true,
        schema_version: Some(info.schema_version),
        migrated_versions: info.migrated_versions,
        safety_copy: Some(info.safety_copy),
    })
}

#[tauri::command]
pub async fn backup_database(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, DbState>,
) -> Result<BackupOutcome, AppError> {
    let db = state.database().cloned().ok_or(AppError::Unavailable)?;
    let default_name = format!("roalt-backup-{}.db", file_stamp(now_ms()));
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Save a backup of the Nexora database")
        .set_file_name(default_name)
        .add_filter("Nexora database backup", &["db"]);
    tauri::async_runtime::spawn_blocking(move || {
        let Some(picked) = dialog.blocking_save_file() else {
            return Ok(BackupOutcome {
                saved: false,
                file_name: None,
                size_bytes: None,
            });
        };
        let dest = picked
            .into_path()
            .map_err(|_| StorageError::Backup("that location cannot be used".into()))?;
        let replace = dest.exists();
        do_backup(&db, &dest, replace, now_ms())
    })
    .await
    .map_err(|_| AppError::Internal)?
    .map_err(AppError::from)
}

#[tauri::command]
pub async fn restore_database(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, DbState>,
) -> Result<RestoreOutcome, AppError> {
    let db = state.database().cloned().ok_or(AppError::Unavailable)?;
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Choose a Nexora backup to restore")
        .add_filter("Nexora database backup", &["db"]);
    tauri::async_runtime::spawn_blocking(move || {
        let Some(picked) = dialog.blocking_pick_file() else {
            return Ok(RestoreOutcome {
                restored: false,
                schema_version: None,
                migrated_versions: vec![],
                safety_copy: None,
            });
        };
        let src = picked
            .into_path()
            .map_err(|_| StorageError::Backup("that file cannot be used".into()))?;
        do_restore(&db, &src, now_ms())
    })
    .await
    .map_err(|_| AppError::Internal)?
    .map_err(AppError::from)
}

pub fn checkpoint_on_exit(state: &DbState) {
    if let Some(db) = state.database()
        && let Err(e) = db.checkpoint()
    {
        eprintln!("[storage] checkpoint on exit failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::TempDir;

    fn kinds(db: &Database) -> Vec<(String, Severity)> {
        db.with(|c| {
            Ok(activity_logs::list(c, &Default::default())?
                .items
                .into_iter()
                .map(|l| (l.event_type, l.severity))
                .collect())
        })
        .unwrap()
    }

    #[test]
    fn first_start_records_real_events_only() {
        let t = TempDir::new();
        let state = init_state(&t.path().join(DB_FILE_NAME), 1_700_000_000_000);
        let db = state.database().expect("available");
        let k = kinds(db);
        // Newest first: started, then the migration that created the schema.
        assert_eq!(
            k,
            [
                ("app.started".to_string(), Severity::Info),
                ("database.migrated".to_string(), Severity::Info)
            ]
        );
        let st = status_of(&state);
        assert!(st.available && st.error.is_none() && st.recovered_file.is_none());
        assert_eq!(
            (st.schema_version, st.journal_mode.as_deref()),
            (Some(1), Some("wal"))
        );
        assert!(st.size_bytes.unwrap() > 0);
    }

    #[test]
    fn second_start_adds_only_a_started_entry() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        drop(init_state(&path, 1_700_000_000_000));
        let state = init_state(&path, 1_700_000_001_000);
        let k = kinds(state.database().unwrap());
        assert_eq!(k.iter().filter(|(t, _)| t == "app.started").count(), 2);
        assert_eq!(
            k.iter().filter(|(t, _)| t == "database.migrated").count(),
            1
        );
    }

    #[test]
    fn recovery_is_reported_in_status_and_the_log() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        std::fs::write(&path, b"garbage garbage garbage").unwrap();
        let state = init_state(&path, 1_700_000_000_000);
        let st = status_of(&state);
        assert!(st.available);
        assert!(st.recovered_file.as_deref().unwrap().starts_with("roalt-"));
        assert!(
            kinds(state.database().unwrap())
                .contains(&("database.recovered".to_string(), Severity::Error))
        );
    }

    #[test]
    fn startup_failure_becomes_state_not_a_crash() {
        let t = TempDir::new();
        let blocker = t.path().join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        let state = init_state(&blocker.join("sub").join(DB_FILE_NAME), 1);
        assert!(state.database().is_none());
        let st = status_of(&state);
        assert!(!st.available);
        assert!(st.error.as_deref().unwrap().contains("data folder"));
        assert!(st.schema_version.is_none());
    }

    #[test]
    fn newer_schema_failure_message_is_clear_and_leaves_the_file() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        drop(init_state(&path, 1));
        Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO schema_migrations VALUES (9,'future','x',1)",
                [],
            )
            .unwrap();
        let state = init_state(&path, 2);
        assert!(state.database().is_none());
        assert!(status_of(&state).error.unwrap().contains("newer version"));
        assert!(path.exists());
    }

    #[test]
    fn failure_messages_never_contain_paths_or_sqlite_text() {
        for e in [
            StorageError::Io(std::io::Error::other("/home/someone/secret")),
            StorageError::Sqlite(rusqlite::Error::InvalidQuery),
            StorageError::Corrupt("page 4".into()),
        ] {
            let m = describe_open_failure(&e);
            assert!(
                !m.contains("secret") && !m.contains("page 4") && !m.contains("Invalid"),
                "{m}"
            );
        }
    }

    #[test]
    fn startup_prunes_entries_past_retention() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        let day = 86_400_000_i64;
        let now = 400 * day;
        {
            let state = init_state(&path, now - 60 * day); // writes entries 60 days "ago"
            assert_eq!(kinds(state.database().unwrap()).len(), 2);
        }
        let state = init_state(&path, now);
        let k = kinds(state.database().unwrap());
        // The 60-day-old entries are gone (default retention is 30 days); new ones exist.
        assert!(k.contains(&("activity.pruned".to_string(), Severity::Debug)));
        assert_eq!(k.iter().filter(|(t, _)| t == "app.started").count(), 1);
    }

    #[test]
    fn backup_and_restore_helpers_log_what_they_did() {
        let t = TempDir::new();
        let state = init_state(&t.path().join(DB_FILE_NAME), 1_700_000_000_000);
        let db = state.database().unwrap();
        let out = TempDir::new();
        let dest = out.path().join("b.db");

        let b = do_backup(db, &dest, false, 1_700_000_001_000).unwrap();
        assert_eq!((b.saved, b.file_name.as_deref()), (true, Some("b.db")));
        assert!(kinds(db).contains(&("database.backup_created".to_string(), Severity::Info)));

        // A second backup to the same name needs the explicit replace flag.
        assert!(matches!(
            do_backup(db, &dest, false, 1_700_000_002_000),
            Err(StorageError::Backup(_))
        ));

        let r = do_restore(db, &dest, 1_700_000_003_000).unwrap();
        assert!(
            r.restored
                && r.safety_copy
                    .as_deref()
                    .unwrap()
                    .starts_with("pre-restore-")
        );
        // The backup was taken before the "backup_created" entry existed, so after restoring it
        // the log shows the restore event on top of the backed-up history.
        let k = kinds(db);
        assert_eq!(k[0], ("database.restored".to_string(), Severity::Warning));
        assert!(!k.iter().any(|(t, _)| t == "database.backup_created"));
    }

    #[test]
    fn dto_types_serialize_in_camel_case() {
        let v = serde_json::to_value(RestoreOutcome {
            restored: true,
            schema_version: Some(1),
            migrated_versions: vec![],
            safety_copy: Some("x".into()),
        })
        .unwrap();
        for key in [
            "restored",
            "schemaVersion",
            "migratedVersions",
            "safetyCopy",
        ] {
            assert!(v.get(key).is_some(), "{key}");
        }
    }
}
