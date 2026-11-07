//! Application settings: a small key/value table plus a registry of the keys the app knows.
//!
//! The table itself accepts any well-formed key (later phases add their own). The commands only
//! accept keys listed in [`KNOWN`], with their allowed range, so the frontend cannot store
//! arbitrary data. A setting that was never saved reads as its default; deleting one resets it.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use ts_rs::TS;

use super::activity_logs::{self, NewActivityLog};
use super::errors::{Result, StorageError};
use super::validation;

/// A setting the app understands. All current settings are whole numbers.
pub struct KnownSetting {
    pub key: &'static str,
    pub default: i64,
    pub min: i64,
    pub max: i64,
}

pub const RETENTION_DAYS: &str = "activity.retention_days";
pub const DASHBOARD_ACTIVITY_COUNT: &str = "ui.dashboard_activity_count";

pub const KNOWN: &[KnownSetting] = &[
    // Activity entries older than this are removed when the app starts.
    KnownSetting {
        key: RETENTION_DAYS,
        default: 30,
        min: 1,
        max: 365,
    },
    // How many recent entries the Dashboard shows.
    KnownSetting {
        key: DASHBOARD_ACTIVITY_COUNT,
        default: 8,
        min: 3,
        max: 25,
    },
];

/// One setting as the frontend sees it: the effective value, whether it is still the default, and
/// the allowed range for the editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingEntry {
    pub key: String,
    pub value: String,
    pub default_value: String,
    pub is_default: bool,
    pub min: i64,
    pub max: i64,
    /// When it was last saved (ms since 1970); `null` while it is still the default.
    pub updated_at: Option<i64>,
}

fn known(key: &str) -> Result<&'static KnownSetting> {
    validation::setting_key(key)?;
    KNOWN
        .iter()
        .find(|k| k.key == key)
        .ok_or_else(|| StorageError::Invalid(format!("unknown setting \"{key}\"")))
}

fn parse_in_range(def: &KnownSetting, raw: &str) -> Result<i64> {
    let n: i64 = raw
        .trim()
        .parse()
        .map_err(|_| StorageError::Invalid(format!("{} must be a whole number", def.key)))?;
    if (def.min..=def.max).contains(&n) {
        Ok(n)
    } else {
        Err(StorageError::Invalid(format!(
            "{} must be between {} and {}",
            def.key, def.min, def.max
        )))
    }
}

// ---- generic key/value layer (any well-formed key) ----

pub fn get_raw(conn: &Connection, key: &str) -> Result<Option<(String, i64)>> {
    validation::setting_key(key)?;
    Ok(conn
        .prepare_cached("SELECT value, updated_at FROM settings WHERE key = ?1")?
        .query_row([key], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?)
}

/// Saves a value. Returns `false` (and writes nothing) when the stored value is already identical,
/// so repeated saves do not cause disk writes or bump `updated_at`.
pub fn set_raw(conn: &Connection, key: &str, value: &str, now_ms: i64) -> Result<bool> {
    validation::setting_key(key)?;
    validation::setting_value(value)?;
    let changed = conn
        .prepare_cached(
            "INSERT INTO settings (key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(key) DO UPDATE
                 SET value = excluded.value, updated_at = excluded.updated_at
                 WHERE settings.value <> excluded.value",
        )?
        .execute(params![key, value, now_ms])?;
    Ok(changed > 0)
}

pub fn delete_raw(conn: &Connection, key: &str) -> Result<bool> {
    validation::setting_key(key)?;
    Ok(conn
        .prepare_cached("DELETE FROM settings WHERE key = ?1")?
        .execute([key])?
        > 0)
}

/// What [`change_raw_logged`] is asked to do once it has seen the value currently stored.
pub(crate) struct RawChange {
    /// The new value, or `None` to remove the setting.
    pub new_value: Option<String>,
    /// The activity entry that records this change.
    pub log: NewActivityLog,
}

/// Changes one setting and records that in the activity log, as a single all-or-nothing step.
///
/// In one transaction it reads the stored value, hands it to `decide`, and then:
/// - `decide` returns `None`: nothing is written or logged, and this returns `false`;
/// - `decide` returns a [`RawChange`]: the value is saved (or removed) and the log entry added,
///   both committed together, and this returns `true`. If either step fails, neither happens.
///
/// `decide` owns the meaning of "unchanged"; this function only guarantees the atomicity. The
/// transaction takes the write lock before the read, so the value `decide` sees cannot be stale.
///
/// Deliberately narrow: it is not a command and is meant to have one caller. Keys in [`KNOWN`]
/// are refused, because they must go through [`set`] to have their range checked.
pub(crate) fn change_raw_logged(
    conn: &mut Connection,
    key: &str,
    now_ms: i64,
    decide: impl FnOnce(Option<&str>) -> Option<RawChange>,
) -> Result<bool> {
    validation::setting_key(key)?;
    if KNOWN.iter().any(|k| k.key == key) {
        return Err(StorageError::Invalid(format!(
            "{key} is a known setting and must be changed with its range checked"
        )));
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let previous = get_raw(&tx, key)?.map(|(value, _)| value);
    let Some(change) = decide(previous.as_deref()) else {
        return Ok(false);
    };
    match &change.new_value {
        Some(value) => {
            set_raw(&tx, key, value, now_ms)?;
        }
        None => {
            delete_raw(&tx, key)?;
        }
    }
    activity_logs::insert_in(&tx, &change.log, now_ms)?;
    tx.commit()?;
    Ok(true)
}

// ---- registry layer (what the commands use) ----

fn entry_from(def: &KnownSetting, stored: Option<(String, i64)>) -> SettingEntry {
    // A stored value that no longer passes the rules (edited by hand, or from a future version)
    // is ignored in favour of the default, and said so in the log, rather than breaking the UI.
    let valid = stored.and_then(|(v, at)| match parse_in_range(def, &v) {
        Ok(_) => Some((v, at)),
        Err(e) => {
            eprintln!("[storage] ignoring stored setting: {e}");
            None
        }
    });
    SettingEntry {
        key: def.key.to_owned(),
        default_value: def.default.to_string(),
        is_default: valid.is_none(),
        min: def.min,
        max: def.max,
        value: valid
            .as_ref()
            .map_or_else(|| def.default.to_string(), |(v, _)| v.trim().to_owned()),
        updated_at: valid.map(|(_, at)| at),
    }
}

pub fn entry(conn: &Connection, key: &str) -> Result<SettingEntry> {
    let def = known(key)?;
    Ok(entry_from(def, get_raw(conn, key)?))
}

/// Every known setting in one query (the Settings page needs them all at once).
pub fn entries(conn: &Connection) -> Result<Vec<SettingEntry>> {
    let mut stmt = conn.prepare_cached("SELECT key, value, updated_at FROM settings")?;
    let stored: Vec<(String, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(KNOWN
        .iter()
        .map(|def| {
            let found = stored
                .iter()
                .find(|(k, _, _)| k == def.key)
                .map(|(_, v, at)| (v.clone(), *at));
            entry_from(def, found)
        })
        .collect())
}

pub fn set(conn: &Connection, key: &str, value: &str, now_ms: i64) -> Result<SettingEntry> {
    let def = known(key)?;
    let n = parse_in_range(def, value)?;
    set_raw(conn, key, &n.to_string(), now_ms)?;
    entry(conn, key)
}

/// Resets a known setting to its default by removing the stored value.
pub fn reset(conn: &Connection, key: &str) -> Result<SettingEntry> {
    known(key)?;
    delete_raw(conn, key)?;
    entry(conn, key)
}

/// The effective integer value of a known setting.
pub fn effective_int(conn: &Connection, key: &str) -> Result<i64> {
    let e = entry(conn, key)?;
    e.value
        .parse()
        .map_err(|_| StorageError::Invalid(format!("{key} is not a number")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::fresh;

    #[test]
    fn raw_crud_round_trip() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert_eq!(get_raw(c, "a.b")?, None);
            assert!(set_raw(c, "a.b", "one", 10)?);
            assert_eq!(get_raw(c, "a.b")?, Some(("one".into(), 10)));
            assert!(set_raw(c, "a.b", "two", 20)?);
            assert_eq!(get_raw(c, "a.b")?, Some(("two".into(), 20)));
            assert!(delete_raw(c, "a.b")?);
            assert!(!delete_raw(c, "a.b")?, "second delete finds nothing");
            assert_eq!(get_raw(c, "a.b")?, None);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn saving_the_same_value_writes_nothing() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert!(set_raw(c, "a.b", "same", 10)?);
            assert!(
                !set_raw(c, "a.b", "same", 99)?,
                "unchanged value is not rewritten"
            );
            assert_eq!(get_raw(c, "a.b")?.unwrap().1, 10, "updated_at did not move");
            let created: i64 =
                c.query_row("SELECT created_at FROM settings WHERE key='a.b'", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(created, 10);
            assert!(set_raw(c, "a.b", "different", 50)?);
            let created: i64 =
                c.query_row("SELECT created_at FROM settings WHERE key='a.b'", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(created, 10, "created_at never changes");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn keys_are_unique_in_the_database_itself() {
        let (_t, db) = fresh();
        db.with(|c| {
            set_raw(c, "dup.key", "1", 1)?;
            let direct = c.execute(
                "INSERT INTO settings (key, value, created_at, updated_at) VALUES ('dup.key','2',1,1)",
                [],
            );
            assert!(direct.is_err(), "UNIQUE(key) must hold even for direct SQL");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn bad_keys_and_values_are_rejected_before_sql() {
        let (_t, db) = fresh();
        db.with(|c| {
            for key in ["", "Bad Key", "x'; DROP TABLE settings;--"] {
                assert!(
                    matches!(set_raw(c, key, "v", 1), Err(StorageError::Invalid(_))),
                    "{key}"
                );
                assert!(matches!(get_raw(c, key), Err(StorageError::Invalid(_))));
                assert!(matches!(delete_raw(c, key), Err(StorageError::Invalid(_))));
            }
            assert!(set_raw(c, "ok.key", &"v".repeat(8193), 1).is_err());
            // The table survived the injection attempt.
            assert!(set_raw(c, "ok.key", "v", 1)?);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn known_settings_default_until_saved() {
        let (_t, db) = fresh();
        db.with(|c| {
            let all = entries(c)?;
            assert_eq!(all.len(), KNOWN.len());
            let r = all.iter().find(|e| e.key == RETENTION_DAYS).unwrap();
            assert_eq!(
                (r.value.as_str(), r.is_default, r.updated_at),
                ("30", true, None)
            );
            assert_eq!((r.min, r.max), (1, 365));

            let saved = set(c, RETENTION_DAYS, "90", 5)?;
            assert_eq!(
                (saved.value.as_str(), saved.is_default, saved.updated_at),
                ("90", false, Some(5))
            );
            assert_eq!(effective_int(c, RETENTION_DAYS)?, 90);

            let back = reset(c, RETENTION_DAYS)?;
            assert_eq!((back.value.as_str(), back.is_default), ("30", true));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn known_settings_enforce_type_and_range() {
        let (_t, db) = fresh();
        db.with(|c| {
            for bad in ["0", "366", "-1", "abc", "", "3.5", "99999999999999999999"] {
                assert!(
                    matches!(
                        set(c, RETENTION_DAYS, bad, 1),
                        Err(StorageError::Invalid(_))
                    ),
                    "{bad:?} should be rejected"
                );
            }
            assert!(matches!(
                set(c, "not.registered", "1", 1),
                Err(StorageError::Invalid(_))
            ));
            assert!(matches!(
                entry(c, "not.registered"),
                Err(StorageError::Invalid(_))
            ));
            assert!(matches!(
                reset(c, "not.registered"),
                Err(StorageError::Invalid(_))
            ));
            assert_eq!(
                set(c, RETENTION_DAYS, " 7 ", 1)?.value,
                "7",
                "whitespace is trimmed"
            );
            // Nothing invalid was stored.
            assert_eq!(get_raw(c, RETENTION_DAYS)?.unwrap().0, "7");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_hand_edited_invalid_stored_value_falls_back_to_the_default() {
        let (_t, db) = fresh();
        db.with(|c| {
            set_raw(c, RETENTION_DAYS, "banana", 1)?;
            let e = entry(c, RETENTION_DAYS)?;
            assert_eq!((e.value.as_str(), e.is_default), ("30", true));
            set_raw(c, DASHBOARD_ACTIVITY_COUNT, "9999", 1)?;
            assert!(entry(c, DASHBOARD_ACTIVITY_COUNT)?.is_default);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn values_survive_closing_and_reopening() {
        let t = crate::storage::test_support::TempDir::new();
        let path = t.path().join("roalt.db");
        {
            let (db, _) = crate::storage::Database::open(&path, 1).unwrap();
            db.with(|c| set(c, DASHBOARD_ACTIVITY_COUNT, "12", 7).map(|_| ()))
                .unwrap();
        }
        let (db, _) = crate::storage::Database::open(&path, 2).unwrap();
        let e = db.with(|c| entry(c, DASHBOARD_ACTIVITY_COUNT)).unwrap();
        assert_eq!((e.value.as_str(), e.updated_at), ("12", Some(7)));
    }

    // ---- `change_raw_logged`: the value and its log entry stand or fall together ----

    use crate::storage::activity_logs::{self, ActivityLogQuery, Severity};

    const KEY: &str = "test.logged_key";

    fn log(kind: &str, message: &str) -> NewActivityLog {
        NewActivityLog {
            event_type: kind.into(),
            severity: Severity::Info,
            message: message.into(),
            metadata_json: Some(r#"{"to":"x"}"#.into()),
        }
    }

    fn change(value: Option<&str>, entry: NewActivityLog) -> Option<RawChange> {
        Some(RawChange {
            new_value: value.map(str::to_owned),
            log: entry,
        })
    }

    fn logged(c: &Connection) -> Vec<(String, String)> {
        activity_logs::list(c, &ActivityLogQuery::default())
            .unwrap()
            .items
            .into_iter()
            .map(|l| (l.event_type, l.message))
            .collect()
    }

    #[test]
    fn a_logged_change_writes_the_value_and_the_entry_together() {
        let (_t, db) = fresh();
        db.with(|c| {
            // Create.
            assert!(change_raw_logged(c, KEY, 10, |prev| {
                assert_eq!(prev, None, "nothing stored yet");
                change(Some("one"), log("t.changed", "created"))
            })?);
            assert_eq!(get_raw(c, KEY)?, Some(("one".into(), 10)));
            // Update: the closure sees what is stored; created_at stays, updated_at moves.
            assert!(change_raw_logged(c, KEY, 20, |prev| {
                assert_eq!(prev, Some("one"));
                change(Some("two"), log("t.changed", "updated"))
            })?);
            assert_eq!(get_raw(c, KEY)?, Some(("two".into(), 20)));
            let created_at: i64 = c.query_row(
                "SELECT created_at FROM settings WHERE key = ?1",
                [KEY],
                |r| r.get(0),
            )?;
            assert_eq!(created_at, 10);
            // The entries are the ones we described, newest first, with their metadata.
            assert_eq!(
                logged(c),
                [
                    ("t.changed".to_string(), "updated".to_string()),
                    ("t.changed".to_string(), "created".to_string())
                ]
            );
            let newest = activity_logs::list(c, &ActivityLogQuery::default())?.items;
            assert_eq!(newest[0].metadata_json.as_deref(), Some(r#"{"to":"x"}"#));
            assert_eq!(
                (newest[0].severity, newest[0].created_at),
                (Severity::Info, 20)
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn deciding_no_change_writes_and_logs_nothing() {
        let (_t, db) = fresh();
        db.with(|c| {
            set_raw(c, KEY, "kept", 100)?;
            assert!(!change_raw_logged(c, KEY, 999, |prev| {
                assert_eq!(prev, Some("kept"));
                None
            })?);
            assert_eq!(
                get_raw(c, KEY)?,
                Some(("kept".into(), 100)),
                "updated_at too"
            );
            assert!(logged(c).is_empty());
            // Same for an absent key.
            assert!(!change_raw_logged(c, "test.absent", 999, |_| None)?);
            assert_eq!(get_raw(c, "test.absent")?, None);
            assert!(logged(c).is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_logged_delete_removes_the_key_and_records_it() {
        let (_t, db) = fresh();
        db.with(|c| {
            set_raw(c, KEY, "gone soon", 5)?;
            assert!(change_raw_logged(c, KEY, 6, |prev| {
                assert_eq!(prev, Some("gone soon"));
                change(None, log("t.reset", "reset"))
            })?);
            assert_eq!(get_raw(c, KEY)?, None);
            assert_eq!(logged(c), [("t.reset".to_string(), "reset".to_string())]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_rejected_log_entry_rolls_back_create_update_and_delete() {
        let (_t, db) = fresh();
        let bad = || log("Bad Type", "x"); // event types must be lowercase dotted names
        db.with(|c| {
            // Create from nothing.
            assert!(change_raw_logged(c, KEY, 1, |_| change(Some("new"), bad())).is_err());
            assert_eq!(get_raw(c, KEY)?, None);
            // Update an existing value.
            set_raw(c, KEY, "old", 2)?;
            assert!(change_raw_logged(c, KEY, 3, |_| change(Some("new"), bad())).is_err());
            assert_eq!(get_raw(c, KEY)?, Some(("old".into(), 2)));
            // Delete an existing value.
            assert!(change_raw_logged(c, KEY, 4, |_| change(None, bad())).is_err());
            assert_eq!(get_raw(c, KEY)?, Some(("old".into(), 2)));
            assert!(logged(c).is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_log_insert_aborted_by_the_database_rolls_back_the_change() {
        let (_t, db) = fresh();
        db.with(|c| {
            set_raw(c, KEY, "old", 2)?;
            c.execute_batch(
                "CREATE TEMP TRIGGER block_insert BEFORE INSERT ON activity_logs
                 BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
            )?;
            assert!(
                change_raw_logged(c, KEY, 3, |_| change(Some("new"), log("t.e", "x"))).is_err()
            );
            assert_eq!(get_raw(c, KEY)?, Some(("old".into(), 2)));
            assert!(logged(c).is_empty());
            // Control: with the trigger gone the same call succeeds, so the trigger was the cause.
            c.execute_batch("DROP TRIGGER block_insert")?;
            assert!(change_raw_logged(c, KEY, 3, |_| change(
                Some("new"),
                log("t.e", "x")
            ))?);
            assert_eq!(get_raw(c, KEY)?, Some(("new".into(), 3)));
            assert_eq!(logged(c).len(), 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn an_invalid_new_value_changes_and_logs_nothing() {
        let (_t, db) = fresh();
        db.with(|c| {
            set_raw(c, KEY, "old", 2)?;
            for bad in ["has\0nul".to_string(), "x".repeat(100_000)] {
                assert!(
                    change_raw_logged(c, KEY, 3, |_| { change(Some(&bad), log("t.e", "x")) })
                        .is_err()
                );
                assert_eq!(get_raw(c, KEY)?, Some(("old".into(), 2)));
                assert!(logged(c).is_empty());
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn bad_keys_and_known_keys_are_refused_before_deciding() {
        let (_t, db) = fresh();
        db.with(|c| {
            for key in ["Bad Key", "", RETENTION_DAYS, DASHBOARD_ACTIVITY_COUNT] {
                let mut asked = false;
                let result = change_raw_logged(c, key, 1, |_| {
                    asked = true;
                    change(Some("1"), log("t.e", "x"))
                });
                assert!(matches!(result, Err(StorageError::Invalid(_))), "{key:?}");
                assert!(!asked, "{key:?}: decide must not run");
            }
            assert_eq!(get_raw(c, RETENTION_DAYS)?, None);
            assert!(logged(c).is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn the_generic_settings_api_cannot_reach_a_key_written_this_way() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert!(change_raw_logged(c, KEY, 1, |_| change(
                Some("secret"),
                log("t.e", "x")
            ))?);
            // `set`, `reset` and `entry` (what the commands call) refuse an unknown key...
            for result in [
                set(c, KEY, "1", 2).map(|_| ()),
                reset(c, KEY).map(|_| ()),
                entry(c, KEY).map(|_| ()),
            ] {
                assert!(matches!(result, Err(StorageError::Invalid(_))));
            }
            // ...the listing shows only the known settings...
            let listed: Vec<_> = entries(c)?.into_iter().map(|e| e.key).collect();
            let known: Vec<_> = KNOWN.iter().map(|k| k.key.to_string()).collect();
            assert_eq!(listed, known);
            // ...the stored value is untouched by those refusals, and known settings still work.
            assert_eq!(get_raw(c, KEY)?, Some(("secret".into(), 1)));
            assert_eq!(set(c, RETENTION_DAYS, "45", 3)?.value, "45");
            Ok(())
        })
        .unwrap();
    }
}
