//! Application settings: a small key/value table plus a registry of the keys the app knows.
//!
//! The table itself accepts any well-formed key (later phases add their own). The commands only
//! accept keys listed in [`KNOWN`], with their allowed range, so the frontend cannot store
//! arbitrary data. A setting that was never saved reads as its default; deleting one resets it.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

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
}
