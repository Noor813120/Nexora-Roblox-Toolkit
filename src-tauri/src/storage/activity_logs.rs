//! Activity history: a bounded, paginated log of things that happened in the app.
//!
//! Bounds: the table never holds more than [`MAX_ROWS`] entries (the oldest are dropped inside the
//! same transaction as each insert), entries older than the retention setting are removed at
//! startup, and a list call returns at most [`MAX_PAGE`] entries. Paging uses "entries older than
//! this id" (keyset paging), which stays fast and stable while new entries arrive.

use rusqlite::{Connection, Row, Transaction, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::errors::{Result, StorageError};
use super::{settings, validation};

/// Hard cap on stored entries.
pub const MAX_ROWS: i64 = 10_000;
pub const DEFAULT_PAGE: u32 = 50;
pub const MAX_PAGE: u32 = 200;
const MS_PER_DAY: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Severity {
    Debug,
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            other => Err(StorageError::Corrupt(format!("unknown severity {other:?}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivityLog {
    pub id: i64,
    pub event_type: String,
    pub severity: Severity,
    pub message: String,
    pub metadata_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewActivityLog {
    pub event_type: String,
    pub severity: Severity,
    pub message: String,
    #[serde(default)]
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivityLogQuery {
    /// Page size; default 50, between 1 and 200.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Return only entries older than this id (the previous page's `nextBeforeId`).
    #[serde(default)]
    pub before_id: Option<i64>,
    #[serde(default)]
    pub severity: Option<Severity>,
    #[serde(default)]
    pub event_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivityLogPage {
    pub items: Vec<ActivityLog>,
    /// Pass this as `beforeId` to get the next (older) page; `null` when there are no more.
    pub next_before_id: Option<i64>,
}

const COLUMNS: &str = "id, event_type, severity, message, metadata_json, created_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<(i64, String, String, String, Option<String>, i64)> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
    ))
}

fn build(raw: (i64, String, String, String, Option<String>, i64)) -> Result<ActivityLog> {
    Ok(ActivityLog {
        id: raw.0,
        event_type: raw.1,
        severity: Severity::parse(&raw.2)?,
        message: raw.3,
        metadata_json: raw.4,
        created_at: raw.5,
    })
}

/// Adds an entry and, in the same transaction, drops anything beyond the cap.
pub fn create(conn: &mut Connection, entry: &NewActivityLog, now_ms: i64) -> Result<ActivityLog> {
    let tx = conn.transaction()?;
    let created = insert_in(&tx, entry, now_ms)?;
    tx.commit()?;
    Ok(created)
}

/// The body of [`create`], run inside a transaction the caller already opened: validates the
/// entry, inserts it, drops anything beyond the cap and reads the row back.
///
/// It never commits. The caller decides, so the entry and the caller's other writes stand or fall
/// together: on any error, dropping the transaction rolls everything back.
pub(super) fn insert_in(
    tx: &Transaction<'_>,
    entry: &NewActivityLog,
    now_ms: i64,
) -> Result<ActivityLog> {
    let event_type = validation::event_type(&entry.event_type)?;
    let message = validation::message(&entry.message)?;
    let metadata = entry
        .metadata_json
        .as_deref()
        .map(|m| validation::json_object(m, "metadata", validation::METADATA_JSON_MAX))
        .transpose()?;

    tx.prepare_cached(
        "INSERT INTO activity_logs (event_type, severity, message, metadata_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(rusqlite::params![
        event_type,
        entry.severity.as_str(),
        message,
        metadata,
        now_ms
    ])?;
    let id = tx.last_insert_rowid();
    // New ids are always max+1 and old entries are only ever removed from the old end, so "the
    // newest MAX_ROWS ids" is exactly "the newest MAX_ROWS entries". One indexed delete, no COUNT.
    tx.prepare_cached("DELETE FROM activity_logs WHERE id <= ?1")?
        .execute([id - MAX_ROWS])?;
    let created = tx
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM activity_logs WHERE id = ?1"
        ))?
        .query_row([id], from_row)?;
    build(created)
}

/// One page, newest first, optionally filtered.
pub fn list(conn: &Connection, query: &ActivityLogQuery) -> Result<ActivityLogPage> {
    let limit = match query.limit {
        None => DEFAULT_PAGE,
        Some(0) => return Err(StorageError::Invalid("limit must be at least 1".into())),
        Some(n) => n.min(MAX_PAGE),
    };
    if let Some(t) = &query.event_type {
        validation::event_type(t)?;
    }

    // The SQL is assembled only from the constant fragments below; every value is a bound
    // parameter, so nothing the caller sends can change the statement's shape.
    let mut sql = format!("SELECT {COLUMNS} FROM activity_logs WHERE 1 = 1");
    let mut args: Vec<Value> = Vec::new();
    if let Some(before) = query.before_id {
        sql.push_str(" AND id < ?");
        args.push(before.into());
    }
    if let Some(sev) = query.severity {
        sql.push_str(" AND severity = ?");
        args.push(sev.as_str().to_owned().into());
    }
    if let Some(t) = &query.event_type {
        sql.push_str(" AND event_type = ?");
        args.push(t.clone().into());
    }
    sql.push_str(" ORDER BY id DESC LIMIT ?");
    args.push((i64::from(limit) + 1).into()); // one extra row tells us whether another page exists

    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params_from_iter(args), from_row)?;
    let mut items = Vec::with_capacity(limit as usize + 1);
    for r in rows {
        items.push(build(r?)?);
    }
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next_before_id = if has_more {
        items.last().map(|l| l.id)
    } else {
        None
    };
    Ok(ActivityLogPage {
        items,
        next_before_id,
    })
}

/// Deletes entries created before `cutoff_ms`. Returns how many were removed.
pub fn clear_before(conn: &Connection, cutoff_ms: i64) -> Result<usize> {
    Ok(conn
        .prepare_cached("DELETE FROM activity_logs WHERE created_at < ?1")?
        .execute([cutoff_ms])?)
}

/// `clear_before` expressed as "older than N days" (1..=3650).
pub fn clear_older_than_days(conn: &Connection, days: u32, now_ms: i64) -> Result<usize> {
    if !(1..=3650).contains(&days) {
        return Err(StorageError::Invalid(
            "days must be between 1 and 3650".into(),
        ));
    }
    clear_before(conn, now_ms - i64::from(days) * MS_PER_DAY)
}

/// Applies the retention setting. Called once at startup.
pub fn prune_by_retention(conn: &Connection, now_ms: i64) -> Result<usize> {
    let days = settings::effective_int(conn, settings::RETENTION_DAYS)?;
    clear_before(conn, now_ms - days * MS_PER_DAY)
}

#[cfg(test)]
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM activity_logs", [], |r| r.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::fresh;

    fn entry(kind: &str, sev: Severity, msg: &str) -> NewActivityLog {
        NewActivityLog {
            event_type: kind.into(),
            severity: sev,
            message: msg.into(),
            metadata_json: None,
        }
    }

    fn q() -> ActivityLogQuery {
        ActivityLogQuery::default()
    }

    #[test]
    fn create_and_list_newest_first() {
        let (_t, db) = fresh();
        db.with(|c| {
            assert!(list(c, &q())?.items.is_empty());
            let a = create(c, &entry("app.started", Severity::Info, " first "), 100)?;
            assert_eq!((a.message.as_str(), a.created_at), ("first", 100));
            create(c, &entry("app.second", Severity::Warning, "second"), 200)?;
            let page = list(c, &q())?;
            assert_eq!(
                page.items
                    .iter()
                    .map(|l| l.message.as_str())
                    .collect::<Vec<_>>(),
                ["second", "first"]
            );
            assert_eq!(page.next_before_id, None);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn metadata_is_stored_when_valid() {
        let (_t, db) = fresh();
        db.with(|c| {
            let with = NewActivityLog {
                metadata_json: Some(r#"{"file":"a.db"}"#.into()),
                ..entry("x.y", Severity::Info, "m")
            };
            let saved = create(c, &with, 1)?;
            assert_eq!(saved.metadata_json.as_deref(), Some(r#"{"file":"a.db"}"#));
            for bad in [
                "nope",
                "[1]",
                &format!("{{\"k\":\"{}\"}}", "x".repeat(20_000)),
            ] {
                let e = NewActivityLog {
                    metadata_json: Some(bad.to_string()),
                    ..entry("x.y", Severity::Info, "m")
                };
                assert!(matches!(create(c, &e, 1), Err(StorageError::Invalid(_))));
            }
            assert_eq!(count(c)?, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn invalid_entries_are_rejected() {
        let (_t, db) = fresh();
        db.with(|c| {
            for bad in [
                entry("", Severity::Info, "m"),
                entry("Bad Type", Severity::Info, "m"),
                entry("ok.type", Severity::Info, "   "),
                entry("ok.type", Severity::Info, &"m".repeat(2001)),
                entry("ok.type", Severity::Info, "bell\u{7}"),
            ] {
                assert!(
                    matches!(create(c, &bad, 1), Err(StorageError::Invalid(_))),
                    "{bad:?}"
                );
            }
            assert_eq!(count(c)?, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn schema_refuses_unknown_severity() {
        let (_t, db) = fresh();
        db.with(|c| {
            let r = c.execute(
                "INSERT INTO activity_logs (event_type, severity, message, created_at) VALUES ('a','fatal','m',1)",
                [],
            );
            assert!(r.is_err());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn pagination_walks_every_entry_exactly_once() {
        let (_t, db) = fresh();
        db.with(|c| {
            for i in 0..23 {
                create(c, &entry("t.e", Severity::Info, &format!("m{i}")), i)?;
            }
            let mut seen = Vec::new();
            let mut cursor = None;
            let mut pages = 0;
            loop {
                let page = list(
                    c,
                    &ActivityLogQuery {
                        limit: Some(10),
                        before_id: cursor,
                        ..q()
                    },
                )?;
                pages += 1;
                seen.extend(page.items.iter().map(|l| l.message.clone()));
                match page.next_before_id {
                    Some(id) => cursor = Some(id),
                    None => break,
                }
            }
            assert_eq!(pages, 3);
            assert_eq!(seen.len(), 23);
            assert_eq!(seen[0], "m22");
            assert_eq!(seen[22], "m0");
            let mut unique = seen.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), 23, "no duplicates across pages");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn exact_multiple_of_page_size_has_no_phantom_next_page() {
        let (_t, db) = fresh();
        db.with(|c| {
            for i in 0..10 {
                create(c, &entry("t.e", Severity::Info, "m"), i)?;
            }
            let page = list(
                c,
                &ActivityLogQuery {
                    limit: Some(10),
                    ..q()
                },
            )?;
            assert_eq!((page.items.len(), page.next_before_id), (10, None));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn page_size_is_bounded() {
        let (_t, db) = fresh();
        db.with(|c| {
            for i in 0..210 {
                create(c, &entry("t.e", Severity::Info, "m"), i)?;
            }
            assert_eq!(list(c, &q())?.items.len(), DEFAULT_PAGE as usize);
            assert_eq!(
                list(
                    c,
                    &ActivityLogQuery {
                        limit: Some(100_000),
                        ..q()
                    }
                )?
                .items
                .len(),
                MAX_PAGE as usize
            );
            assert!(matches!(
                list(
                    c,
                    &ActivityLogQuery {
                        limit: Some(0),
                        ..q()
                    }
                ),
                Err(StorageError::Invalid(_))
            ));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn filters_by_severity_and_event_type() {
        let (_t, db) = fresh();
        db.with(|c| {
            create(c, &entry("db.backup", Severity::Info, "a"), 1)?;
            create(c, &entry("db.restore", Severity::Warning, "b"), 2)?;
            create(c, &entry("db.backup", Severity::Error, "c"), 3)?;
            let msgs = |query: ActivityLogQuery| -> Result<Vec<String>> {
                Ok(list(c, &query)?
                    .items
                    .into_iter()
                    .map(|l| l.message)
                    .collect())
            };
            assert_eq!(
                msgs(ActivityLogQuery {
                    severity: Some(Severity::Error),
                    ..q()
                })?,
                ["c"]
            );
            assert_eq!(
                msgs(ActivityLogQuery {
                    event_type: Some("db.backup".into()),
                    ..q()
                })?,
                ["c", "a"]
            );
            assert_eq!(
                msgs(ActivityLogQuery {
                    event_type: Some("db.backup".into()),
                    severity: Some(Severity::Info),
                    ..q()
                })?,
                ["a"]
            );
            assert!(
                msgs(ActivityLogQuery {
                    event_type: Some("nothing.here".into()),
                    ..q()
                })?
                .is_empty()
            );
            assert!(matches!(
                list(
                    c,
                    &ActivityLogQuery {
                        event_type: Some("x'; DROP TABLE activity_logs;--".into()),
                        ..q()
                    }
                ),
                Err(StorageError::Invalid(_))
            ));
            assert_eq!(count(c)?, 3);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_never_exceeds_the_cap() {
        let (_t, db) = fresh();
        db.with(|c| {
            let tx = c.transaction()?;
            // Fill to the cap quickly with one bulk insert (inside a transaction), then use the
            // real `create` path to push past it.
            tx.execute(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?1)
                 INSERT INTO activity_logs (event_type, severity, message, created_at)
                 SELECT 'bulk.fill', 'info', 'row ' || i, i FROM n",
                [MAX_ROWS],
            )?;
            tx.commit()?;
            assert_eq!(count(c)?, MAX_ROWS);
            for i in 0..5 {
                create(
                    c,
                    &entry("t.new", Severity::Info, &format!("new{i}")),
                    99_999 + i,
                )?;
            }
            assert_eq!(
                count(c)?,
                MAX_ROWS,
                "oldest entries were dropped to stay at the cap"
            );
            let newest = list(c, &q())?.items;
            assert_eq!(newest[0].message, "new4");
            let oldest: String = c.query_row(
                "SELECT message FROM activity_logs ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(oldest, "row 6", "rows 1-5 were trimmed");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn clear_old_removes_only_old_entries() {
        let (_t, db) = fresh();
        let day = MS_PER_DAY;
        db.with(|c| {
            let now = 100 * day;
            create(c, &entry("t.e", Severity::Info, "ancient"), now - 40 * day)?;
            create(c, &entry("t.e", Severity::Info, "old"), now - 10 * day)?;
            create(c, &entry("t.e", Severity::Info, "fresh"), now - day / 2)?;
            assert_eq!(clear_older_than_days(c, 30, now)?, 1);
            assert_eq!(count(c)?, 2);
            assert_eq!(clear_older_than_days(c, 1, now)?, 1);
            assert_eq!(list(c, &q())?.items[0].message, "fresh");
            assert_eq!(
                clear_older_than_days(c, 1, now)?,
                0,
                "nothing left to clear"
            );
            assert!(matches!(
                clear_older_than_days(c, 0, now),
                Err(StorageError::Invalid(_))
            ));
            assert!(matches!(
                clear_older_than_days(c, 99_999, now),
                Err(StorageError::Invalid(_))
            ));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn retention_setting_controls_startup_pruning() {
        let (_t, db) = fresh();
        let day = MS_PER_DAY;
        db.with(|c| {
            let now = 500 * day;
            create(
                c,
                &entry("t.e", Severity::Info, "40 days old"),
                now - 40 * day,
            )?;
            create(
                c,
                &entry("t.e", Severity::Info, "5 days old"),
                now - 5 * day,
            )?;
            // Default retention is 30 days.
            assert_eq!(prune_by_retention(c, now)?, 1);
            settings::set(c, settings::RETENTION_DAYS, "2", now)?;
            assert_eq!(prune_by_retention(c, now)?, 1);
            assert_eq!(count(c)?, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn indexes_serve_the_filtered_and_cleanup_queries() {
        let (_t, db) = fresh();
        db.with(|c| {
            let plan = |sql: &str| -> Result<String> {
                let mut s = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
                let rows: Vec<String> = s
                    .query_map([], |r| r.get::<_, String>(3))?
                    .collect::<std::result::Result<_, _>>()?;
                Ok(rows.join(" | "))
            };
            let sev = plan(
                "SELECT id FROM activity_logs WHERE severity = 'error' ORDER BY id DESC LIMIT 10",
            )?;
            assert!(sev.contains("idx_activity_logs_severity"), "{sev}");
            let typ = plan(
                "SELECT id FROM activity_logs WHERE event_type = 'a.b' ORDER BY id DESC LIMIT 10",
            )?;
            assert!(typ.contains("idx_activity_logs_event_type"), "{typ}");
            let old = plan("DELETE FROM activity_logs WHERE created_at < 5")?;
            assert!(old.contains("idx_activity_logs_created_at"), "{old}");
            let newest = plan("SELECT id FROM activity_logs ORDER BY id DESC LIMIT 10")?;
            assert!(
                !newest.contains("TEMP B-TREE"),
                "newest-first must not need a sort: {newest}"
            );
            Ok(())
        })
        .unwrap();
    }

    // ---- `insert_in`: the entry stands or falls with the caller's transaction ----

    /// A caller that writes a setting and then logs, in one transaction (the shape Stage 2 uses).
    fn write_then_log(c: &mut Connection, e: &NewActivityLog, commit: bool) -> Result<()> {
        let tx = c.transaction()?;
        settings::set_raw(&tx, "test.marker", "set", 1)?;
        insert_in(&tx, e, 2)?;
        if commit {
            tx.commit()?;
        }
        Ok(())
    }

    fn marker(c: &Connection) -> Option<String> {
        settings::get_raw(c, "test.marker").unwrap().map(|(v, _)| v)
    }

    #[test]
    fn helper_does_not_commit_the_callers_transaction() {
        let (_t, db) = fresh();
        db.with(|c| {
            // Caller drops the transaction without committing: nothing may remain.
            write_then_log(c, &entry("t.e", Severity::Info, "x"), false)?;
            assert_eq!(count(c)?, 0, "the helper must not have committed the entry");
            assert_eq!(marker(c), None, "nor the caller's own write");
            // Control: when the caller does commit, both are there.
            write_then_log(c, &entry("t.e", Severity::Info, "x"), true)?;
            assert_eq!(count(c)?, 1);
            assert_eq!(marker(c).as_deref(), Some("set"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_rejected_entry_rolls_back_the_callers_write() {
        let (_t, db) = fresh();
        db.with(|c| {
            for bad in [
                entry("Bad Type", Severity::Info, "x"), // event type
                entry("t.e", Severity::Info, "   "),    // empty message
                NewActivityLog {
                    metadata_json: Some("[1]".into()), // not a JSON object
                    ..entry("t.e", Severity::Info, "x")
                },
            ] {
                assert!(write_then_log(c, &bad, true).is_err());
                assert_eq!(count(c)?, 0);
                assert_eq!(marker(c), None, "the caller's write was rolled back too");
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn an_insert_aborted_by_the_database_rolls_back_the_callers_write() {
        let (_t, db) = fresh();
        db.with(|c| {
            c.execute_batch(
                "CREATE TEMP TRIGGER block_insert BEFORE INSERT ON activity_logs
                 BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
            )?;
            assert!(write_then_log(c, &entry("t.e", Severity::Info, "x"), true).is_err());
            assert_eq!(count(c)?, 0);
            assert_eq!(marker(c), None);
            // Control: the same call succeeds once the trigger is gone, so the failure above
            // really came from the trigger.
            c.execute_batch("DROP TRIGGER block_insert")?;
            write_then_log(c, &entry("t.e", Severity::Info, "x"), true)?;
            assert_eq!((count(c)?, marker(c).as_deref()), (1, Some("set")));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn failed_cap_pruning_rolls_back_the_entry_and_the_callers_write() {
        let (_t, db) = fresh();
        db.with(|c| {
            let tx = c.transaction()?;
            tx.execute(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?1)
                 INSERT INTO activity_logs (event_type, severity, message, created_at)
                 SELECT 'bulk.fill', 'info', 'row ' || i, i FROM n",
                [MAX_ROWS],
            )?;
            tx.commit()?;
            let first_id = |c: &Connection| -> Result<i64> {
                Ok(c.query_row("SELECT MIN(id) FROM activity_logs", [], |r| r.get(0))?)
            };

            // At the cap, the next insert must delete row 1. Make that delete fail.
            c.execute_batch(
                "CREATE TEMP TRIGGER block_prune BEFORE DELETE ON activity_logs
                 BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
            )?;
            assert!(write_then_log(c, &entry("t.new", Severity::Info, "new"), true).is_err());
            assert_eq!(count(c)?, MAX_ROWS, "the inserted entry was rolled back");
            assert_eq!(first_id(c)?, 1, "nothing was pruned");
            assert_eq!(marker(c), None, "the caller's write was rolled back too");

            // Control: without the trigger the same call prunes exactly one row and commits.
            c.execute_batch("DROP TRIGGER block_prune")?;
            write_then_log(c, &entry("t.new", Severity::Info, "new"), true)?;
            assert_eq!(count(c)?, MAX_ROWS);
            assert_eq!(first_id(c)?, 2);
            assert_eq!(marker(c).as_deref(), Some("set"));
            Ok(())
        })
        .unwrap();
    }
}
