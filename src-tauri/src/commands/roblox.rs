//! Roblox commands.
//!
//! - [`detect_roblox`] (Milestone 1): a read-only check for an installed Roblox player. Its result
//!   goes to the screen only. It is deliberately not written to the activity log or the database,
//!   and it takes no database state at all.
//! - [`get_install_selection`] and [`set_preferred_install`] (Milestone 2): the same detection,
//!   plus the user's preferred installation. Only choosing a preference writes anything, and only
//!   to Nexora's own database; Roblox's files and registry are never written.
//!
//! The Tauri commands are thin. The work is in [`selection_with`] and [`apply_with`], which take
//! the computer ([`Environment`]) and the database as plain arguments so tests can supply both.
//! Detection is always done *before* the database lock is taken, so file and registry reads never
//! hold up the database. It is a point-in-time observation: anything that later acts on it (such
//! as launching) must check again immediately before acting.

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use crate::commands::database::DbState;
use crate::error::AppError;
use crate::launcher::preference::{
    EVENT_TYPE, PreferenceResolution, PreferenceState, PreferredSource, SETTING_KEY,
    StoredPreference, available_sources, parse_stored, plan_change, resolve,
};
use crate::launcher::roblox::{DetectionStatus, Environment, RobloxDetection, detect};
use crate::platform::roblox::SystemEnvironment;
use crate::storage::Database;
use crate::storage::activity_logs::{NewActivityLog, Severity};
use crate::storage::clock::now_ms;
use crate::storage::settings::{self, RawChange};

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

// ---------------------------------------------------------------------------------------------
// Preferred installation (Milestone 2)
// ---------------------------------------------------------------------------------------------

/// What the Game Launcher shows: what was detected, and what the stored preference means against
/// it. Both come from one detection, so they can never contradict each other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallSelection {
    pub detection: RobloxDetection,
    /// `None` only when the preference could not be read (the database is unavailable). That is
    /// not the same as "automatic", and the screen must not present it as such.
    pub preference: Option<PreferenceResolution>,
    /// The sources that would be accepted as a choice right now, from this same detection, in a
    /// fixed order (link handler, then version folders). The choice control offers only these.
    /// Empty on an unsupported system.
    pub available_sources: Vec<PreferredSource>,
}

/// The user's choice, as the screen sends it. Deliberately a required three-way value rather than
/// an optional source: an omitted or misspelt argument is an error, never a silent reset to
/// "automatic".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstallChoice {
    Automatic,
    ProtocolHandler,
    VersionFolder,
}

impl InstallChoice {
    fn into_target(self) -> Option<PreferredSource> {
        match self {
            Self::Automatic => None,
            Self::ProtocolHandler => Some(PreferredSource::ProtocolHandler),
            Self::VersionFolder => Some(PreferredSource::VersionFolder),
        }
    }
}

/// Detects, then reads the stored preference and compares the two. Writes nothing.
///
/// If the preference cannot be read (no database, or a read error) the detection is still
/// returned, with `preference: None`.
pub(crate) fn selection_with(env: &dyn Environment, db: Option<&Database>) -> InstallSelection {
    let detection = detect(env);
    let preference = db.and_then(|db| match db.with(|c| settings::get_raw(c, SETTING_KEY)) {
        Ok(stored) => Some(resolve(
            &detection,
            parse_stored(stored.as_ref().map(|(value, _)| value.as_str())),
        )),
        Err(e) => {
            eprintln!("[roblox] could not read the preferred installation: {e}");
            None
        }
    });
    InstallSelection {
        available_sources: available_sources(&detection),
        detection,
        preference,
    }
}

/// Saves the user's choice and records it in the activity log, all or nothing.
///
/// Order matters: detection runs first, *before* the database lock is taken, so the choice is
/// checked against what is on the computer right now. Choosing a source that does not currently
/// resolve is rejected, and then nothing is stored or logged. "Automatic" (`None`) is always
/// allowed. A choice that changes nothing writes and logs nothing.
///
/// The returned selection uses the same detection and the value as stored under the same lock.
pub(crate) fn apply_with(
    env: &dyn Environment,
    db: Option<&Database>,
    target: Option<PreferredSource>,
    now: i64,
) -> Result<InstallSelection, AppError> {
    let db = db.ok_or(AppError::Unavailable)?;
    let detection = detect(env);
    if let Some(source) = target {
        if detection.status == DetectionStatus::Unsupported {
            return Err(AppError::Conflict(
                "Roblox installations can only be selected on Windows.".into(),
            ));
        }
        if resolve(&detection, StoredPreference::Source(source)).state != PreferenceState::Available
        {
            return Err(AppError::Conflict(
                "That Roblox source is not available right now. Check again.".into(),
            ));
        }
    }
    let stored = db
        .with(|c| {
            settings::change_raw_logged(c, SETTING_KEY, now, |previous| {
                let change = plan_change(previous, target)?;
                Some(RawChange {
                    log: NewActivityLog {
                        event_type: EVENT_TYPE.into(),
                        severity: Severity::Info,
                        message: change.message.clone(),
                        metadata_json: Some(change.metadata_json()),
                    },
                    new_value: change.new_raw,
                })
            })?;
            Ok(settings::get_raw(c, SETTING_KEY)?.map(|(value, _)| value))
        })
        .map_err(AppError::from)?;
    let preference = resolve(&detection, parse_stored(stored.as_deref()));
    Ok(InstallSelection {
        available_sources: available_sources(&detection),
        detection,
        preference: Some(preference),
    })
}

/// Detection plus the preferred installation. Read-only.
#[tauri::command]
pub async fn get_install_selection(
    state: State<'_, DbState>,
) -> Result<InstallSelection, AppError> {
    let db = state.database().cloned();
    tauri::async_runtime::spawn_blocking(move || selection_with(&SystemEnvironment, db.as_deref()))
        .await
        .map_err(|e| {
            eprintln!("[roblox] selection task failed: {e}");
            AppError::Internal
        })
}

/// Saves the preferred installation. The only way the preference changes. It never touches
/// Roblox's files or registry; it writes Nexora's own database.
#[tauri::command]
pub async fn set_preferred_install(
    state: State<'_, DbState>,
    choice: InstallChoice,
) -> Result<InstallSelection, AppError> {
    let db = state.database().cloned();
    tauri::async_runtime::spawn_blocking(move || {
        apply_with(
            &SystemEnvironment,
            db.as_deref(),
            choice.into_target(),
            now_ms(),
        )
    })
    .await
    .map_err(|e| {
        eprintln!("[roblox] selection task failed: {e}");
        AppError::Internal
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::preference::{PreferenceReason, canonical};
    use crate::launcher::roblox::tests::{Fake, file};
    use crate::launcher::roblox::{DetectionStatus, DirListing, FileState, RegistryValue};
    use crate::storage::test_support::{TempDir, fresh};
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

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

    // ---- preferred installation ----

    const PH: PreferredSource = PreferredSource::ProtocolHandler;
    const VF: PreferredSource = PreferredSource::VersionFolder;
    const A: &str = "version-aaaaaaaaaaaaaaaa";
    const B: &str = "version-bbbbbbbbbbbbbbbb";
    const C: &str = "version-cccccccccccccccc";
    const ROOT: &str = r"C:\Roblox\Versions";
    const SET_VF: &str = "Preferred Roblox installation set to Roblox version folders.";
    const SET_PH: &str = "Preferred Roblox installation set to Windows link handler.";
    const RESET: &str = "Preferred Roblox installation reset to automatic.";
    const NOT_AVAILABLE: &str = "That Roblox source is not available right now. Check again.";
    const WINDOWS_ONLY: &str = "Roblox installations can only be selected on Windows.";

    fn machine() -> Fake {
        let mut env = Fake::windows();
        env.versions = Some(PathBuf::from(ROOT));
        env
    }

    /// Both sources resolve: two folders, and the link handler points at the newer one.
    fn good_machine() -> Fake {
        let env = machine()
            .with_folders(&[A, B])
            .with_exe(A, file(Some(100)))
            .with_exe(B, file(Some(200)));
        let handler = env.exe(B).to_string_lossy().into_owned();
        env.with_handler(RegistryValue::Text(handler))
    }

    /// Only the version-folder source resolves.
    fn folders_only() -> Fake {
        machine().with_folders(&[A]).with_exe(A, file(Some(1)))
    }

    fn off() -> Fake {
        let mut env = Fake::windows();
        env.supported = false;
        env
    }

    /// Only the link-handler source resolves: it points at a program outside any version folder.
    fn handler_only() -> Fake {
        let launcher = r"C:\Other\RobloxPlayerLauncher.exe";
        machine()
            .with_handler(RegistryValue::Text(launcher.to_string()))
            .with_file(launcher, file(Some(1)))
    }

    /// Usable version folders, but the link handler cannot be read: only that source is lost.
    fn folders_with_unreadable_handler() -> Fake {
        folders_only().with_handler(RegistryValue::Inaccessible)
    }

    /// A usable link handler, but the Versions folder cannot be read: only that source is lost.
    fn handler_with_unreadable_versions() -> Fake {
        let mut env = handler_only();
        env.dirs
            .insert(PathBuf::from(ROOT), DirListing::Inaccessible);
        env
    }

    /// (event type, severity, message, metadata), newest first.
    type Event = (String, String, String, Option<String>);

    fn events(db: &Database) -> Vec<Event> {
        db.with(|c| {
            let mut stmt = c.prepare(
                "SELECT event_type, severity, message, metadata_json FROM activity_logs ORDER BY id DESC",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<Vec<Event>, rusqlite::Error>>()?;
            Ok(rows)
        })
        .unwrap()
    }

    /// Only the preference-changed entries, newest first.
    fn changes(db: &Database) -> Vec<Event> {
        events(db)
            .into_iter()
            .filter(|e| e.0 == EVENT_TYPE)
            .collect()
    }

    fn stored(db: &Database) -> Option<String> {
        db.with(|c| Ok(settings::get_raw(c, SETTING_KEY)?.map(|(value, _)| value)))
            .unwrap()
    }

    /// The stored value together with its `updated_at`, straight from the database.
    fn stored_row(db: &Database) -> Option<(String, i64)> {
        db.with(|c| settings::get_raw(c, SETTING_KEY)).unwrap()
    }

    fn set_stored(db: &Database, raw: Option<&str>) {
        db.with(|c| match raw {
            Some(r) => settings::set_raw(c, SETTING_KEY, r, 1).map(|_| ()),
            None => settings::delete_raw(c, SETTING_KEY).map(|_| ()),
        })
        .unwrap();
    }

    fn state_of(selection: &InstallSelection) -> PreferenceState {
        selection.preference.as_ref().expect("a preference").state
    }

    fn conflict(result: Result<InstallSelection, AppError>) -> String {
        match result {
            Err(AppError::Conflict(message)) => message,
            other => panic!("expected a conflict, got {other:?}"),
        }
    }

    // ---- reading ----

    #[test]
    fn reading_shows_each_stored_state_and_never_changes_detection() {
        let (_t, db) = fresh();
        let env = good_machine();
        let expected = detect(&env);
        let cases = [
            (None, PreferenceState::Automatic),
            (Some(canonical(PH)), PreferenceState::Available),
            (Some(canonical(VF)), PreferenceState::Available),
            (Some("garbage".to_string()), PreferenceState::Invalid),
            (
                Some(r#"{"v":2,"source":"versionFolder"}"#.to_string()),
                PreferenceState::Unsupported,
            ),
            (
                Some(r#"{"v":1,"source":"microsoftStore"}"#.to_string()),
                PreferenceState::Unsupported,
            ),
        ];
        for (raw, state) in cases {
            set_stored(&db, raw.as_deref());
            let before = stored_row(&db);
            assert_eq!(
                before.as_ref().map(|(value, _)| value.as_str()),
                raw.as_deref(),
                "the value under test is exactly what was seeded"
            );
            let selection = selection_with(&env, Some(&db));
            assert_eq!(selection.detection, expected, "{raw:?}");
            assert_eq!(state_of(&selection), state, "{raw:?}");
            // Reading left the stored bytes and their timestamp exactly as they were, and an
            // absent value is still absent.
            assert_eq!(stored_row(&db), before, "{raw:?}");
        }
        assert!(
            events(&db).is_empty(),
            "reading writes nothing, not even a log entry"
        );
    }

    #[test]
    fn without_a_readable_preference_detection_still_comes_back_and_is_not_called_automatic() {
        let env = good_machine();
        let expected = detect(&env);

        let none = selection_with(&env, None);
        assert_eq!(none.detection, expected);
        assert_eq!(none.preference, None);
        let json = serde_json::to_value(&none).unwrap();
        assert_eq!(json["preference"], Value::Null);
        assert_eq!(json["detection"]["status"], "detected");

        // A database that is there but cannot be read gives the same answer.
        let (_t, db) = fresh();
        db.with(|c| {
            c.execute_batch("ALTER TABLE settings RENAME TO settings_gone")?;
            Ok(())
        })
        .unwrap();
        let broken = selection_with(&env, Some(&db));
        assert_eq!(broken.detection, expected);
        assert_eq!(broken.preference, None);
    }

    // ---- writing ----

    #[test]
    fn choosing_a_source_stores_it_and_logs_once() {
        let (_t, db) = fresh();
        let env = good_machine();

        let selection = apply_with(&env, Some(&db), Some(VF), 10).unwrap();
        let preference = selection.preference.clone().unwrap();
        assert_eq!(
            (
                preference.state,
                preference.preferred,
                preference.candidate_count
            ),
            (PreferenceState::Available, Some(VF), 2)
        );
        assert_eq!(
            selection.detection,
            detect(&env),
            "same detection as a plain read"
        );
        assert_eq!(stored(&db).as_deref(), Some(canonical(VF).as_str()));
        let log = changes(&db);
        assert_eq!(log.len(), 1);
        assert_eq!((log[0].1.as_str(), log[0].2.as_str()), ("info", SET_VF));
        let meta: Value = serde_json::from_str(log[0].3.as_deref().unwrap()).unwrap();
        assert_eq!(meta, json!({"from": "automatic", "to": "versionFolder"}));

        // The same choice again changes and logs nothing, and does not touch updated_at.
        let again = apply_with(&env, Some(&db), Some(VF), 99).unwrap();
        assert_eq!(state_of(&again), PreferenceState::Available);
        assert_eq!(changes(&db).len(), 1);
        let updated_at = db
            .with(|c| Ok(settings::get_raw(c, SETTING_KEY)?.unwrap().1))
            .unwrap();
        assert_eq!(updated_at, 10);

        // Switching source is a change.
        apply_with(&env, Some(&db), Some(PH), 20).unwrap();
        let log = changes(&db);
        assert_eq!((log.len(), log[0].2.as_str()), (2, SET_PH));
        let meta: Value = serde_json::from_str(log[0].3.as_deref().unwrap()).unwrap();
        assert_eq!(
            meta,
            json!({"from": "versionFolder", "to": "protocolHandler"})
        );

        // "Automatic" removes the setting and logs it; doing it again changes nothing.
        let auto = apply_with(&env, Some(&db), None, 30).unwrap();
        assert_eq!(state_of(&auto), PreferenceState::Automatic);
        assert_eq!(
            auto.preference.unwrap().chosen.as_ref(),
            auto.detection.installs.first()
        );
        assert_eq!(stored(&db), None);
        assert_eq!((changes(&db).len(), changes(&db)[0].2.as_str()), (3, RESET));
        apply_with(&env, Some(&db), None, 40).unwrap();
        assert_eq!(changes(&db).len(), 3);
    }

    #[test]
    fn an_equivalent_stored_value_is_not_rewritten_or_logged() {
        let (_t, db) = fresh();
        let env = good_machine();

        // The same meaning as the canonical text, written differently: other key order, extra
        // spaces and an extra field.
        let spaced_vf = r#"{ "source" : "versionFolder" ,  "extra": [1, 2], "v" : 1 }"#;
        set_stored(&db, Some(spaced_vf));
        let before = stored_row(&db);
        let again = apply_with(&env, Some(&db), Some(VF), 99).unwrap();
        assert_eq!(state_of(&again), PreferenceState::Available);
        assert_eq!(stored_row(&db), before, "same bytes, same updated_at");
        assert_eq!(stored(&db).as_deref(), Some(spaced_vf));
        assert!(
            events(&db).is_empty(),
            "nothing changed, so nothing is logged"
        );

        // Control: a different choice does rewrite it, in canonical form, and says where it
        // came from. So the no-op above is about meaning, not about never writing.
        apply_with(&env, Some(&db), Some(PH), 100).unwrap();
        assert_eq!(stored(&db).as_deref(), Some(canonical(PH).as_str()));
        let log = changes(&db);
        assert_eq!(log.len(), 1);
        let meta: Value = serde_json::from_str(log[0].3.as_deref().unwrap()).unwrap();
        assert_eq!(
            meta,
            json!({"from": "versionFolder", "to": "protocolHandler"})
        );

        // The same for the other source.
        let spaced_ph = r#"{"v":1,  "source":"protocolHandler"  }"#;
        set_stored(&db, Some(spaced_ph));
        let before = stored_row(&db);
        apply_with(&env, Some(&db), Some(PH), 101).unwrap();
        assert_eq!(stored_row(&db), before);
        assert_eq!(changes(&db).len(), 1, "still only the one real change");
    }

    #[test]
    fn a_source_that_does_not_resolve_is_rejected_and_changes_nothing() {
        let (_t, db) = fresh();

        // Nothing stored: a source the machine lacks is refused, and nothing is written.
        assert_eq!(
            conflict(apply_with(&folders_only(), Some(&db), Some(PH), 1)),
            NOT_AVAILABLE
        );
        // "Could not tell" is not "available" either.
        let unreadable_handler = machine()
            .with_folders(&[A])
            .with_exe(A, file(Some(1)))
            .with_handler(RegistryValue::Inaccessible);
        assert_eq!(
            conflict(apply_with(&unreadable_handler, Some(&db), Some(PH), 1)),
            NOT_AVAILABLE
        );
        let mut unreadable_versions = machine();
        unreadable_versions
            .dirs
            .insert(PathBuf::from(ROOT), DirListing::Inaccessible);
        assert_eq!(
            conflict(apply_with(&unreadable_versions, Some(&db), Some(VF), 1)),
            NOT_AVAILABLE
        );
        assert_eq!(stored(&db), None);
        assert!(events(&db).is_empty());

        // An existing preference survives a rejected change.
        apply_with(&good_machine(), Some(&db), Some(VF), 2).unwrap();
        assert_eq!(
            conflict(apply_with(&folders_only(), Some(&db), Some(PH), 3)),
            NOT_AVAILABLE
        );
        assert_eq!(stored(&db).as_deref(), Some(canonical(VF).as_str()));
        assert_eq!(changes(&db).len(), 1);
    }

    #[test]
    fn on_an_unsupported_system_sources_are_rejected_but_automatic_still_clears() {
        let (_t, db) = fresh();

        // Nothing stored: automatic is a no-op.
        let selection = apply_with(&off(), Some(&db), None, 1).unwrap();
        assert_eq!(state_of(&selection), PreferenceState::Automatic);
        assert!(events(&db).is_empty());

        for source in [PH, VF] {
            assert_eq!(
                conflict(apply_with(&off(), Some(&db), Some(source), 2)),
                WINDOWS_ONLY
            );
        }
        assert_eq!(stored(&db), None);
        assert!(events(&db).is_empty());

        // A preference that arrived some other way (a restored backup) can still be cleared.
        set_stored(&db, Some(&canonical(VF)));
        for source in [PH, VF] {
            assert_eq!(
                conflict(apply_with(&off(), Some(&db), Some(source), 3)),
                WINDOWS_ONLY
            );
        }
        assert_eq!(stored(&db).as_deref(), Some(canonical(VF).as_str()));
        let cleared = apply_with(&off(), Some(&db), None, 4).unwrap();
        assert_eq!(state_of(&cleared), PreferenceState::Automatic);
        assert_eq!(stored(&db), None);
        let log = changes(&db);
        assert_eq!((log.len(), log[0].2.as_str()), (1, RESET));
    }

    #[test]
    fn without_a_database_choosing_is_unavailable_and_nothing_is_attempted() {
        for target in [None, Some(PH), Some(VF)] {
            let result = apply_with(&good_machine(), None, target, 1);
            assert!(
                matches!(result, Err(AppError::Unavailable)),
                "{target:?}: {result:?}"
            );
        }
    }

    #[test]
    fn a_failed_log_insert_fails_the_whole_change() {
        let (_t, db) = fresh();
        let env = good_machine();
        apply_with(&env, Some(&db), Some(PH), 1).unwrap();
        db.with(|c| {
            c.execute_batch(
                "CREATE TEMP TRIGGER block_insert BEFORE INSERT ON activity_logs
                 BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
            )?;
            Ok(())
        })
        .unwrap();

        // The storage layer classifies a database constraint abort as a conflict and passes
        // SQLite's own message through, so this is exactly the trigger's error: it did not come
        // from either of the checks, and it is not some other kind of failure.
        let result = apply_with(&env, Some(&db), Some(VF), 2);
        assert_eq!(conflict(result), "blocked");
        assert_eq!(
            stored(&db).as_deref(),
            Some(canonical(PH).as_str()),
            "unchanged"
        );
        assert_eq!(changes(&db).len(), 1);

        // Control: with the trigger gone the same call succeeds.
        db.with(|c| {
            c.execute_batch("DROP TRIGGER block_insert")?;
            Ok(())
        })
        .unwrap();
        apply_with(&env, Some(&db), Some(VF), 2).unwrap();
        assert_eq!(stored(&db).as_deref(), Some(canonical(VF).as_str()));
        assert_eq!(changes(&db).len(), 2);
    }

    #[test]
    fn concurrent_writers_leave_a_consistent_history() {
        let (_t, db) = fresh();
        let env = good_machine();
        let targets = [Some(PH), Some(VF), None];
        std::thread::scope(|scope| {
            for t in 0..4usize {
                let (db, env) = (&db, &env);
                scope.spawn(move || {
                    for i in 0..25usize {
                        let target = targets[(t * 7 + i * 3 + i / 2) % 3];
                        apply_with(env, Some(db), target, (t * 100 + i) as i64)
                            .expect("a choice that resolves always succeeds");
                    }
                });
            }
        });

        // Oldest first: every entry must start from where the previous one ended.
        let mut log = changes(&db);
        log.reverse();
        assert!(!log.is_empty() && log.len() <= 100, "{}", log.len());
        let mut at = "automatic".to_string();
        for entry in &log {
            let meta: Value = serde_json::from_str(entry.3.as_deref().unwrap()).unwrap();
            assert_eq!(
                meta["from"],
                at.as_str(),
                "the history has a gap or an overlap"
            );
            assert_ne!(
                meta["from"], meta["to"],
                "an unchanged choice must not be logged"
            );
            at = meta["to"].as_str().unwrap().to_string();
        }
        let expected = match at.as_str() {
            "automatic" => None,
            "protocolHandler" => Some(canonical(PH)),
            "versionFolder" => Some(canonical(VF)),
            other => panic!("unexpected label {other}"),
        };
        assert_eq!(
            stored(&db),
            expected,
            "the stored value is where the history ends"
        );
    }

    // ---- order: detection first, database lock second ----

    /// A computer that, every time it is asked something, checks from another thread whether the
    /// database lock is free. If detection ran while the lock was held, that thread would be stuck.
    struct LockProbe {
        inner: Fake,
        db: Arc<Database>,
        free: std::cell::RefCell<Vec<bool>>,
    }

    impl LockProbe {
        fn can_lock(db: &Arc<Database>, wait: Duration) -> bool {
            let (tx, rx) = std::sync::mpsc::channel();
            let db = Arc::clone(db);
            std::thread::spawn(move || {
                let _ = db.with(|_| Ok(()));
                let _ = tx.send(());
            });
            rx.recv_timeout(wait).is_ok()
        }

        fn note(&self) {
            let free = Self::can_lock(&self.db, Duration::from_secs(2));
            self.free.borrow_mut().push(free);
        }
    }

    impl Environment for LockProbe {
        fn platform_supported(&self) -> bool {
            self.note();
            self.inner.platform_supported()
        }
        fn versions_dir(&self) -> Option<PathBuf> {
            self.note();
            self.inner.versions_dir()
        }
        fn list_dirs(&self, dir: &Path) -> DirListing {
            self.note();
            self.inner.list_dirs(dir)
        }
        fn file_state(&self, path: &Path) -> FileState {
            self.note();
            self.inner.file_state(path)
        }
        fn protocol_handler_command(&self) -> RegistryValue {
            self.note();
            self.inner.protocol_handler_command()
        }
    }

    #[test]
    fn detection_happens_before_the_database_lock_is_taken() {
        let (_t, db) = fresh();
        let db = Arc::new(db);
        let probe = LockProbe {
            inner: good_machine(),
            db: Arc::clone(&db),
            free: Default::default(),
        };

        apply_with(&probe, Some(&db), Some(VF), 1).unwrap();
        let seen = probe.free.borrow().clone();
        assert!(
            seen.len() >= 5,
            "detection asked the computer {} times",
            seen.len()
        );
        assert!(
            seen.iter().all(|free| *free),
            "the lock was held during detection: {seen:?}"
        );

        probe.free.borrow_mut().clear();
        selection_with(&probe, Some(&db));
        let seen = probe.free.borrow().clone();
        assert!(seen.len() >= 5);
        assert!(
            seen.iter().all(|free| *free),
            "the lock was held during detection: {seen:?}"
        );

        // Control: the probe really can tell when the lock is held.
        let blocked = db
            .with(|_| Ok(LockProbe::can_lock(&db, Duration::from_millis(300))))
            .unwrap();
        assert!(
            !blocked,
            "the probe must notice a held lock, or the test above proves nothing"
        );
    }

    // ---- how many times detection runs ----

    /// A computer that counts how many detections start. `detect` asks `platform_supported`
    /// exactly once per run, before anything else, so that call marks one detection. (The first
    /// test below checks that assumption rather than trusting it.)
    struct Counting {
        inner: Fake,
        detections: std::cell::Cell<usize>,
    }

    impl Counting {
        fn new(inner: Fake) -> Self {
            Counting {
                inner,
                detections: Default::default(),
            }
        }
    }

    impl Environment for Counting {
        fn platform_supported(&self) -> bool {
            self.detections.set(self.detections.get() + 1);
            self.inner.platform_supported()
        }
        fn versions_dir(&self) -> Option<PathBuf> {
            self.inner.versions_dir()
        }
        fn list_dirs(&self, dir: &Path) -> DirListing {
            self.inner.list_dirs(dir)
        }
        fn file_state(&self, path: &Path) -> FileState {
            self.inner.file_state(path)
        }
        fn protocol_handler_command(&self) -> RegistryValue {
            self.inner.protocol_handler_command()
        }
    }

    /// Runs `operation` on a counting copy of `machine` and says how many detections it started.
    fn detections(machine: Fake, operation: impl FnOnce(&Counting)) -> usize {
        let env = Counting::new(machine);
        operation(&env);
        env.detections.get()
    }

    #[test]
    fn every_operation_detects_exactly_once_and_a_missing_database_detects_never() {
        // The sentinel itself: one plain detection counts as exactly one.
        assert_eq!(detections(good_machine(), |env| drop(detect(env))), 1);

        let (_t, db) = fresh();

        // Choosing: a real change, the same choice again (no change), and a switch.
        let db_ref = &db;
        let choose = |target, now| {
            move |env: &Counting| {
                apply_with(env, Some(db_ref), target, now).unwrap();
            }
        };
        assert_eq!(
            detections(good_machine(), choose(Some(VF), 1)),
            1,
            "a real change"
        );
        assert_eq!(
            detections(good_machine(), choose(Some(VF), 2)),
            1,
            "no change"
        );
        assert_eq!(
            detections(good_machine(), choose(Some(PH), 3)),
            1,
            "a switch"
        );
        assert_eq!(detections(good_machine(), choose(None, 4)), 1, "automatic");
        assert_eq!(
            detections(good_machine(), choose(None, 5)),
            1,
            "automatic, no change"
        );

        // Rejected choices detect once too, whichever check refuses them.
        assert_eq!(
            detections(folders_only(), |env| {
                assert!(apply_with(env, Some(&db), Some(PH), 6).is_err());
            }),
            1,
            "a source that does not resolve"
        );
        assert_eq!(
            detections(off(), |env| {
                assert!(apply_with(env, Some(&db), Some(VF), 7).is_err());
            }),
            1,
            "an unsupported system"
        );

        // A change that fails inside the database still detected exactly once.
        db.with(|c| {
            c.execute_batch(
                "CREATE TEMP TRIGGER block_insert BEFORE INSERT ON activity_logs
                 BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            detections(good_machine(), |env| {
                assert!(apply_with(env, Some(&db), Some(VF), 8).is_err());
            }),
            1,
            "a change whose log entry fails"
        );

        // Reading: once, with or without a database.
        assert_eq!(
            detections(good_machine(), |env| drop(selection_with(env, Some(&db)))),
            1
        );
        assert_eq!(
            detections(good_machine(), |env| drop(selection_with(env, None))),
            1
        );

        // No database: choosing stops before any detection, for every target.
        for target in [None, Some(PH), Some(VF)] {
            assert_eq!(
                detections(good_machine(), |env| {
                    assert!(matches!(
                        apply_with(env, None, target, 9),
                        Err(AppError::Unavailable)
                    ));
                }),
                0,
                "{target:?} without a database"
            );
        }
    }

    // ---- which sources can be chosen ----

    /// Builds a fresh pretend computer, so one test can use the same machine more than once.
    type Maker = fn() -> Fake;

    #[test]
    fn the_selection_lists_the_sources_from_its_one_detection() {
        let (_t, db) = fresh();
        let cases: [(&str, Maker, Vec<PreferredSource>); 5] = [
            ("both", good_machine, vec![PH, VF]),
            ("folders only", folders_only, vec![VF]),
            ("handler only", handler_only, vec![PH]),
            ("unsupported", off, vec![]),
            ("nothing", machine, vec![]),
        ];
        for (name, make, expected) in cases {
            // Reading: the list comes from the one detection the read already makes.
            let mut read = None;
            let count = detections(make(), |env| read = Some(selection_with(env, Some(&db))));
            assert_eq!(
                (read.unwrap().available_sources, count),
                (expected.clone(), 1),
                "{name}: read"
            );

            // With no database the preference cannot be read, but the sources are still listed.
            let mut blind = None;
            let count = detections(make(), |env| blind = Some(selection_with(env, None)));
            let blind = blind.unwrap();
            assert_eq!(
                (blind.preference, blind.available_sources, count),
                (None, expected.clone(), 1),
                "{name}: no database"
            );

            // And in what choosing returns ("automatic" is accepted on every machine).
            let mut chosen = None;
            let count = detections(make(), |env| {
                chosen = Some(apply_with(env, Some(&db), None, 1).unwrap());
            });
            assert_eq!(
                (chosen.unwrap().available_sources, count),
                (expected, 1),
                "{name}: choosing"
            );
        }
    }

    #[test]
    fn a_source_is_listed_if_and_only_if_choosing_it_is_accepted() {
        let machines: [(&str, Maker); 8] = [
            ("both", good_machine),
            ("folders only", folders_only),
            ("handler only", handler_only),
            ("unsupported", off),
            ("nothing", machine),
            (
                "folders, handler unreadable",
                folders_with_unreadable_handler,
            ),
            (
                "handler, versions unreadable",
                handler_with_unreadable_versions,
            ),
            ("nothing, versions unreadable", || {
                let mut env = machine();
                env.dirs
                    .insert(PathBuf::from(ROOT), DirListing::Inaccessible);
                env
            }),
        ];
        for (name, make) in machines {
            for source in [PH, VF] {
                let (_t, db) = fresh();
                let listed = selection_with(&make(), Some(&db))
                    .available_sources
                    .contains(&source);
                let accepted = apply_with(&make(), Some(&db), Some(source), 1).is_ok();
                assert_eq!(listed, accepted, "{name} / {source:?}");
            }
        }

        // The two mixed cases, concretely: one unreadable source does not suppress the other.
        let (_t, db) = fresh();
        let lists = |env: Fake| selection_with(&env, Some(&db)).available_sources;
        assert_eq!(lists(folders_with_unreadable_handler()), vec![VF]);
        assert_eq!(lists(handler_with_unreadable_versions()), vec![PH]);
    }

    #[test]
    fn the_listed_sources_cross_the_boundary_in_order_and_by_name() {
        let both = serde_json::to_value(selection_with(&good_machine(), None)).unwrap();
        assert_eq!(
            both["availableSources"],
            json!(["protocolHandler", "versionFolder"])
        );
        let none = serde_json::to_value(selection_with(&off(), None)).unwrap();
        assert_eq!(none["availableSources"], json!([]));
    }

    #[test]
    fn the_log_holds_exactly_the_agreed_text_and_never_a_path() {
        let (_t, db) = fresh();
        let env = good_machine();
        // Every expected value is written out here as a literal, on purpose: none of it is taken
        // from the constants the code under test uses, so changing one of those constants cannot
        // change this test's expectation along with it.
        let entry = |message: &str, metadata: &str| {
            (
                "launcher.preferred_install_changed".to_string(),
                "info".to_string(),
                message.to_string(),
                Some(metadata.to_string()),
            )
        };
        let first = entry(
            "Preferred Roblox installation set to Roblox version folders.",
            r#"{"from":"automatic","to":"versionFolder"}"#,
        );
        let second = entry(
            "Preferred Roblox installation set to Windows link handler.",
            r#"{"from":"versionFolder","to":"protocolHandler"}"#,
        );
        let third = entry(
            "Preferred Roblox installation reset to automatic.",
            r#"{"from":"protocolHandler","to":"automatic"}"#,
        );

        // Whole entries, compared in full: no other entry, no extra field, no other text.
        apply_with(&env, Some(&db), Some(VF), 1).unwrap();
        assert_eq!(events(&db), vec![first.clone()]);
        apply_with(&env, Some(&db), Some(PH), 2).unwrap();
        assert_eq!(events(&db), vec![second.clone(), first.clone()]);
        apply_with(&env, Some(&db), None, 3).unwrap();
        assert_eq!(events(&db), vec![third, second, first]);

        // The stored value is the canonical text and nothing else.
        apply_with(&env, Some(&db), Some(VF), 4).unwrap();
        assert_eq!(
            stored(&db).as_deref(),
            Some(r#"{"v":1,"source":"versionFolder"}"#)
        );
    }

    // ---- backups ----

    #[test]
    fn restoring_a_backup_keeps_the_preference_exactly_and_logs_no_change() {
        let t = TempDir::new();
        let open = |name: &str| Database::open(&t.path().join(name), 1).unwrap().0;
        let (a, b) = (open("a.db"), open("b.db"));

        // "Machine A" chooses, then backs up.
        apply_with(&good_machine(), Some(&a), Some(VF), 10).unwrap();
        let backup = t.path().join("a.backup.db");
        a.backup_to(&backup, false).unwrap();

        // "Machine B" restores it.
        assert_eq!(changes(&b).len(), 0);
        b.restore_from(&backup, 20).unwrap();
        assert_eq!(stored(&b), stored(&a));
        assert_eq!(stored(&b).as_deref(), Some(canonical(VF).as_str()));
        assert_eq!(
            changes(&b).len(),
            1,
            "only the entry A wrote: restoring adds none"
        );
        // Restoring replaces the data and adds nothing of its own (the app's restore command
        // writes its own "database.restored" entry separately, and that is not a preference change).
        assert_eq!(events(&b), events(&a));

        // The restored preference is judged against what machine B actually has.
        let mut unreadable = machine();
        unreadable
            .dirs
            .insert(PathBuf::from(ROOT), DirListing::Inaccessible);
        let many = machine()
            .with_folders(&[A, B, C])
            .with_exe(A, file(Some(1)))
            .with_exe(B, file(Some(3)))
            .with_exe(C, file(Some(2)));
        for (name, env, state, count) in [
            (
                "source present",
                good_machine(),
                PreferenceState::Available,
                2,
            ),
            ("source absent", machine(), PreferenceState::Unavailable, 0),
            ("several folders", many, PreferenceState::Available, 3),
            (
                "could not tell",
                unreadable,
                PreferenceState::Inconclusive,
                0,
            ),
        ] {
            let selection = selection_with(&env, Some(&b));
            let preference = selection.preference.unwrap();
            assert_eq!(
                (preference.state, preference.candidate_count),
                (state, count),
                "{name}"
            );
            assert_eq!(preference.preferred, Some(VF), "{name}");
        }

        // An older backup that never had a preference restores to "automatic".
        let c = open("c.db");
        let old = t.path().join("c.backup.db");
        c.backup_to(&old, false).unwrap();
        b.restore_from(&old, 30).unwrap();
        assert_eq!(stored(&b), None);
        let selection = selection_with(&good_machine(), Some(&b));
        assert_eq!(state_of(&selection), PreferenceState::Automatic);
    }

    #[test]
    fn a_newer_format_value_survives_backup_and_restore_byte_for_byte() {
        let t = TempDir::new();
        let open = |name: &str| Database::open(&t.path().join(name), 1).unwrap().0;
        let newer = r#"{"v":2,"source":"versionFolder","extra":"keep me"}"#;
        let a = open("a.db");
        set_stored(&a, Some(newer));

        let first = t.path().join("first.backup.db");
        a.backup_to(&first, false).unwrap();
        let d = open("d.db");
        d.restore_from(&first, 50).unwrap();
        assert_eq!(stored(&d).as_deref(), Some(newer));
        let before_read = stored_row(&d);
        let preference = selection_with(&good_machine(), Some(&d))
            .preference
            .unwrap();
        // The read itself left the value, and its timestamp, exactly as they were.
        assert_eq!(stored_row(&d), before_read);
        assert_eq!(stored(&d).as_deref(), Some(newer));
        assert_eq!(
            (preference.state, preference.reason),
            (
                PreferenceState::Unsupported,
                Some(PreferenceReason::NewerFormat)
            )
        );

        // And again, through a second backup and a third database.
        let second = t.path().join("second.backup.db");
        d.backup_to(&second, false).unwrap();
        let e = open("e.db");
        e.restore_from(&second, 60).unwrap();
        assert_eq!(stored(&e).as_deref(), Some(newer), "byte for byte");
        assert_eq!(changes(&d).len(), 0);
        assert_eq!(
            changes(&e).len(),
            0,
            "restoring never logs a preference change"
        );
    }

    // ---- the command's argument ----

    #[test]
    fn the_choice_type_rejects_null_and_invented_values() {
        for bad in [
            Value::Null,
            json!("reset"),
            json!("Automatic"),
            json!(""),
            json!({"source": "versionFolder"}),
            json!(["automatic"]),
            json!(1),
        ] {
            assert!(
                serde_json::from_value::<InstallChoice>(bad.clone()).is_err(),
                "{bad} must not be accepted"
            );
        }
        for (text, target) in [
            ("automatic", None),
            ("protocolHandler", Some(PH)),
            ("versionFolder", Some(VF)),
        ] {
            let choice: InstallChoice = serde_json::from_value(json!(text)).unwrap();
            assert_eq!(choice.into_target(), target);
        }
    }

    // ---- the IPC boundary ----

    /// A harmless command with the same argument shape. It only proves the harness delivers
    /// well-formed requests, so "rejected" below cannot mean "nothing is ever delivered".
    #[tauri::command]
    fn boundary_control(choice: String) -> usize {
        choice.len()
    }

    /// Every column of every row of the two tables the command could touch, tagged by table.
    fn full_snapshot(db: &Database) -> Vec<String> {
        db.with(|c| {
            let mut out = Vec::new();
            for (table, sql) in [
                ("settings", "SELECT * FROM settings ORDER BY id"),
                ("activity_logs", "SELECT * FROM activity_logs ORDER BY id"),
            ] {
                let mut stmt = c.prepare(sql)?;
                let columns = stmt.column_count();
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let cells = (0..columns)
                        .map(|i| {
                            row.get_ref(i).map(|v| match v {
                                rusqlite::types::ValueRef::Text(t) => {
                                    format!("{:?}", String::from_utf8_lossy(t))
                                }
                                other => format!("{other:?}"),
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    out.push(format!("{table}: {}", cells.join(" | ")));
                }
            }
            Ok(out)
        })
        .unwrap()
    }

    /// Existing data the command must leave alone, including a saved preference: choosing
    /// "automatic" would delete it and log a change, so a request that wrongly got through as
    /// "automatic" shows up in the snapshot (it would be invisible against empty tables).
    fn seed_existing_data(db: &Database) {
        db.with(|c| {
            for (key, value, created, updated) in [
                ("activity.retention_days", "30", 100_i64, 200_i64),
                ("ui.dashboard_activity_count", "8", 110, 210),
                (SETTING_KEY, canonical(VF).as_str(), 120, 220),
            ] {
                c.execute(
                    "INSERT INTO settings (key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![key, value, created, updated],
                )?;
            }
            for (event, severity, message, metadata, created) in [
                ("app.started", "info", "Nexora 0.1.0 started.", None, 300_i64),
                (EVENT_TYPE, "info", SET_VF, Some(r#"{"from":"automatic","to":"versionFolder"}"#), 310),
                ("database.cleaned_up", "warning", "Removed 3 old entries.", Some(r#"{"removed":3}"#), 320),
            ] {
                c.execute(
                    "INSERT INTO activity_logs (event_type, severity, message, metadata_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![event, severity, message, metadata, created],
                )?;
            }
            Ok(())
        })
        .unwrap();
    }

    /// Test-only IPC command using a deterministic Roblox environment.
    #[tauri::command]
    fn test_set_preferred_install(
        state: State<'_, DbState>,
        choice: InstallChoice,
    ) -> Result<InstallSelection, AppError> {
        let env = good_machine();
        let db = state.database().cloned();

        apply_with(&env, db.as_deref(), choice.into_target(), now_ms())
    }

    #[test]
    fn malformed_ipc_requests_are_rejected_and_change_nothing() {
        use tauri::Manager;
        use tauri::ipc::{CallbackFn, InvokeBody};
        use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
        use tauri::webview::InvokeRequest;
        // Declared first, so it is dropped last.
        let dir = TempDir::new();
        let state = crate::commands::database::init_state(&dir.path().join("ipc.db"), 1_000);
        let db = state
            .database()
            .cloned()
            .expect("the temporary database opens");
        seed_existing_data(&db);
        let before = full_snapshot(&db);

        let rows_of = |table: &str| before.iter().filter(|r| r.starts_with(table)).count();
        assert_eq!(rows_of("settings:"), 3, "{before:#?}");
        assert!(rows_of("activity_logs:") > 3, "{before:#?}");
        assert!(
            before
                .iter()
                .any(|r| r.starts_with("settings:") && r.contains("versionFolder")),
            "the saved preference is part of the snapshot"
        );

        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                test_set_preferred_install,
                boundary_control
            ])
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        app.manage(state);
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("the mock webview builds");
        let send = |cmd: &str, body: InvokeBody| {
            get_ipc_response(
                &webview,
                InvokeRequest {
                    cmd: cmd.into(),
                    callback: CallbackFn(0),
                    error: CallbackFn(1),
                    url: if cfg!(windows) {
                        "http://tauri.localhost"
                    } else {
                        "tauri://localhost"
                    }
                    .parse()
                    .unwrap(),
                    body,
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            )
        };

        // Control: a well-formed request to the harmless command is delivered and answered.
        let control = send(
            "boundary_control",
            InvokeBody::Json(json!({"choice": "abc"})),
        );
        assert_eq!(
            control.ok().and_then(|b| b.deserialize::<usize>().ok()),
            Some(3)
        );

        let rejected: [(&str, InvokeBody); 14] = [
            ("missing choice", InvokeBody::Json(json!({}))),
            ("no body", InvokeBody::default()),
            ("null body", InvokeBody::Json(Value::Null)),
            ("array body", InvokeBody::Json(json!([]))),
            ("choice null", InvokeBody::Json(json!({"choice": null}))),
            (
                "invented string",
                InvokeBody::Json(json!({"choice": "reset"})),
            ),
            (
                "wrong casing of a value",
                InvokeBody::Json(json!({"choice": "Automatic"})),
            ),
            ("empty string", InvokeBody::Json(json!({"choice": ""}))),
            ("number", InvokeBody::Json(json!({"choice": 1}))),
            ("object", InvokeBody::Json(json!({"choice": {}}))),
            ("array", InvokeBody::Json(json!({"choice": ["automatic"]}))),
            (
                "wrong key casing",
                InvokeBody::Json(json!({"Choice": "automatic"})),
            ),
            (
                "wrong key name",
                InvokeBody::Json(json!({"source": "versionFolder"})),
            ),
            (
                "invented string with an extra key",
                InvokeBody::Json(json!({"choice": "reset", "extra": 1})),
            ),
        ];
        for (name, body) in rejected {
            let response = send("test_set_preferred_install", body);
            assert!(response.is_err(), "{name}: the request was accepted");
            assert_eq!(full_snapshot(&db), before, "{name}: the database changed");
        }

        // Still delivering well-formed requests after all of that.
        // Valid choices must pass through real Tauri IPC and persist correctly.
        let valid_choices = [
            ("protocolHandler", Some(PH)),
            ("versionFolder", Some(VF)),
            ("automatic", None),
        ];

        for (choice, expected) in valid_choices {
            let response = send(
                "test_set_preferred_install",
                InvokeBody::Json(json!({"choice": choice})),
            );

            assert!(
                response.is_ok(),
                "valid choice {choice} was rejected: {response:?}"
            );

            let expected_stored = expected.map(canonical);
            assert_eq!(
                stored(&db),
                expected_stored,
                "wrong persisted preference for {choice}"
            );
        }
        let again = send(
            "boundary_control",
            InvokeBody::Json(json!({"choice": "abcd"})),
        );
        assert_eq!(
            again.ok().and_then(|b| b.deserialize::<usize>().ok()),
            Some(4)
        );

        // Release every handle this test holds on the database.
        drop(webview);
        drop(app);
        drop(db);
        // Known limit: on Windows the mock app keeps its managed database state open until the
        // test process exits, so `dir` usually cannot be removed here and a small folder named
        // `roalt-adv-test-*` stays in the temp directory. Nothing outside it is touched.
    }
}
