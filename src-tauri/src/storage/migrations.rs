//! Versioned schema migrations.
//!
//! How it works, in plain words: every change to the database structure is a numbered SQL file
//! that is built into the app. The `schema_migrations` table remembers which numbers have already
//! been applied (and when). On startup the app applies the missing ones, in order, each inside its
//! own transaction, so a migration either fully happens or not at all.
//!
//! Safety rules enforced here:
//! - A migration that was already applied must still match what is built in (its checksum is
//!   stored), so history cannot be silently rewritten.
//! - A database that is *newer* than this app is never touched.
//! - Applied versions must form an unbroken run 1..=N.
//!
//! The checksum is FNV-1a 64-bit of the SQL text. It detects accidental edits; it is not a security
//! feature.

use rusqlite::{Connection, params};

use super::errors::{Result, StorageError};

#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Every migration this build knows, oldest first. Add new ones at the end; never edit or reorder
/// an existing entry once it has shipped.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial",
    sql: include_str!("migrations/001_initial.sql"),
}];

/// What is applied and what is waiting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub applied: Vec<u32>,
    pub pending: Vec<u32>,
}

pub fn checksum(sql: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in sql.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn ensure_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             name       TEXT    NOT NULL,
             checksum   TEXT    NOT NULL,
             applied_at INTEGER NOT NULL
         );",
    )?;
    Ok(())
}

/// Checks the list itself (versions 1,2,3... with no gaps) so a developer mistake fails loudly.
fn validate_list(migrations: &[Migration]) -> Result<()> {
    for (i, m) in migrations.iter().enumerate() {
        let expected = u32::try_from(i + 1).unwrap_or(u32::MAX);
        if m.version != expected {
            return Err(StorageError::Invalid(format!(
                "migration list is not 1..N: found {} at position {}",
                m.version,
                i + 1
            )));
        }
    }
    Ok(())
}

/// Reads the history, verifies it against `migrations` and says what is left to do. Changes
/// nothing except creating the bookkeeping table.
pub fn plan(conn: &Connection, migrations: &[Migration]) -> Result<Plan> {
    validate_list(migrations)?;
    ensure_table(conn)?;
    let mut stmt =
        conn.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
    let rows: Vec<(u32, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;

    let supported = u32::try_from(migrations.len()).unwrap_or(u32::MAX);
    let mut applied = Vec::with_capacity(rows.len());
    for (i, (version, stored)) in rows.iter().enumerate() {
        if *version > supported {
            return Err(StorageError::SchemaTooNew {
                found: *version,
                supported,
            });
        }
        let expected = u32::try_from(i + 1).unwrap_or(u32::MAX);
        if *version != expected {
            return Err(StorageError::Corrupt(format!(
                "migration history has a gap: expected {expected}, found {version}"
            )));
        }
        if *stored != checksum(migrations[i].sql) {
            return Err(StorageError::MigrationMismatch { version: *version });
        }
        applied.push(*version);
    }
    let pending = migrations
        .iter()
        .map(|m| m.version)
        .filter(|v| !applied.contains(v))
        .collect();
    Ok(Plan { applied, pending })
}

/// Applies every pending migration. Each runs in its own transaction together with its history
/// row; on any error that transaction is rolled back and the database stays at the last good
/// version. Returns the versions applied by this call.
pub fn migrate(conn: &mut Connection, migrations: &[Migration], now_ms: i64) -> Result<Vec<u32>> {
    let plan = plan(conn, migrations)?;
    let mut done = Vec::new();
    for version in plan.pending {
        let m = &migrations[(version - 1) as usize];
        let tx = conn.transaction()?;
        let outcome = tx.execute_batch(m.sql).and_then(|()| {
            tx.execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![m.version, m.name, checksum(m.sql), now_ms],
            )
            .map(|_| ())
        });
        match outcome {
            Ok(()) => {
                tx.commit()?;
                done.push(version);
            }
            Err(e) => {
                // Dropping `tx` rolls the half-applied migration back.
                return Err(StorageError::MigrationFailed {
                    version,
                    message: e.to_string(),
                });
            }
        }
    }
    Ok(done)
}

/// The newest version recorded in the database (0 if none).
pub fn current_version(conn: &Connection) -> Result<u32> {
    ensure_table(conn)?;
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1: Migration = Migration {
        version: 1,
        name: "one",
        sql: "CREATE TABLE a (id INTEGER PRIMARY KEY, x TEXT);",
    };
    const V2: Migration = Migration {
        version: 2,
        name: "two",
        sql: "ALTER TABLE a ADD COLUMN y TEXT NOT NULL DEFAULT 'new';",
    };
    const V3_BAD: Migration = Migration {
        version: 3,
        name: "three",
        // The first statement works, the second fails: the whole migration must roll back.
        sql: "CREATE TABLE b (id INTEGER); INSERT INTO does_not_exist VALUES (1);",
    };

    fn mem() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn checksum_is_stable_and_sensitive() {
        assert_eq!(checksum("abc"), checksum("abc"));
        assert_ne!(checksum("abc"), checksum("abd"));
        assert_eq!(checksum("").len(), 16);
    }

    #[test]
    fn real_migration_list_is_well_formed() {
        validate_list(MIGRATIONS).unwrap();
        assert!(MIGRATIONS.iter().all(|m| !m.sql.trim().is_empty()));
    }

    #[test]
    fn fresh_database_gets_everything_and_records_it() {
        let mut c = mem();
        let applied = migrate(&mut c, &[V1, V2], 1234).unwrap();
        assert_eq!(applied, vec![1, 2]);
        assert_eq!(current_version(&c).unwrap(), 2);
        let (name, at): (String, i64) = c
            .query_row(
                "SELECT name, applied_at FROM schema_migrations WHERE version = 2",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((name.as_str(), at), ("two", 1234));
    }

    #[test]
    fn running_twice_does_nothing_the_second_time() {
        let mut c = mem();
        migrate(&mut c, &[V1, V2], 1).unwrap();
        assert!(migrate(&mut c, &[V1, V2], 2).unwrap().is_empty());
        let rows: i64 = c
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 2);
    }

    #[test]
    fn upgrade_keeps_existing_data() {
        let mut c = mem();
        migrate(&mut c, &[V1], 1).unwrap();
        c.execute("INSERT INTO a (x) VALUES ('keep me')", [])
            .unwrap();
        assert_eq!(migrate(&mut c, &[V1, V2], 2).unwrap(), vec![2]);
        let (x, y): (String, String) = c
            .query_row("SELECT x, y FROM a", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((x.as_str(), y.as_str()), ("keep me", "new"));
    }

    #[test]
    fn failing_migration_rolls_back_completely() {
        let mut c = mem();
        migrate(&mut c, &[V1, V2], 1).unwrap();
        c.execute("INSERT INTO a (x) VALUES ('safe')", []).unwrap();
        let err = migrate(&mut c, &[V1, V2, V3_BAD], 2).unwrap_err();
        assert!(
            matches!(err, StorageError::MigrationFailed { version: 3, .. }),
            "{err}"
        );
        // Still at version 2, table b was not left behind, data untouched.
        assert_eq!(current_version(&c).unwrap(), 2);
        let b: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'b'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(b, 0);
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM a", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn newer_database_is_refused_and_untouched() {
        let mut c = mem();
        migrate(&mut c, &[V1, V2], 1).unwrap();
        let err = migrate(&mut c, &[V1], 2).unwrap_err();
        assert!(
            matches!(
                err,
                StorageError::SchemaTooNew {
                    found: 2,
                    supported: 1
                }
            ),
            "{err}"
        );
        assert_eq!(current_version(&c).unwrap(), 2);
    }

    #[test]
    fn edited_migration_is_detected() {
        let mut c = mem();
        migrate(&mut c, &[V1], 1).unwrap();
        let edited = Migration {
            sql: "CREATE TABLE a (id INTEGER PRIMARY KEY, x TEXT, sneaky TEXT);",
            ..V1
        };
        let err = migrate(&mut c, &[edited], 2).unwrap_err();
        assert!(
            matches!(err, StorageError::MigrationMismatch { version: 1 }),
            "{err}"
        );
    }

    #[test]
    fn gap_in_history_is_reported_as_corruption() {
        let c = mem();
        ensure_table(&c).unwrap();
        c.execute(
            "INSERT INTO schema_migrations VALUES (2, 'two', ?1, 1)",
            [checksum(V2.sql)],
        )
        .unwrap();
        let err = plan(&c, &[V1, V2]).unwrap_err();
        assert!(err.is_corruption(), "{err}");
    }

    #[test]
    fn malformed_list_is_rejected() {
        let skipped = Migration { version: 3, ..V2 };
        assert!(matches!(
            plan(&mem(), &[V1, skipped]).unwrap_err(),
            StorageError::Invalid(_)
        ));
    }

    #[test]
    fn real_schema_creates_the_expected_tables_and_indexes() {
        let mut c = mem();
        migrate(&mut c, MIGRATIONS, 1).unwrap();
        let names = |kind: &str| -> Vec<String> {
            let mut s = c
                .prepare("SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite_%' ORDER BY name")
                .unwrap();
            s.query_map([kind], |r| r.get(0))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap()
        };
        assert_eq!(
            names("table"),
            [
                "activity_logs",
                "automation_profiles",
                "profiles",
                "schema_migrations",
                "settings"
            ]
        );
        let idx = names("index");
        for expected in [
            "idx_activity_logs_created_at",
            "idx_activity_logs_event_type",
            "idx_activity_logs_severity",
            "idx_profiles_name",
            "idx_profiles_single_default",
            "idx_automation_profiles_name",
        ] {
            assert!(idx.iter().any(|n| n == expected), "missing {expected}");
        }
    }
}
