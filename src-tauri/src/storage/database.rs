//! Opening the database safely: location, connection settings, identity check, integrity check,
//! migrations and recovery from a damaged file.
//!
//! One connection lives behind a mutex. For a single-user desktop app with small, quick queries
//! that is simpler and safer than a pool (no pool bugs, one place that owns the file), and SQLite
//! serialises writers anyway. Callers run work through [`Database::with`]; the Tauri commands
//! call it from a blocking thread so the async runtime never waits on disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::Connection;

use super::clock::file_stamp;
use super::errors::{Result, StorageError};
use super::migrations::{self, MIGRATIONS, Migration};

/// File name inside the data folder.
pub const DB_FILE_NAME: &str = "roalt.db";
/// Folder name under the OS local-data directory (`%LOCALAPPDATA%\RoALT-Advanced` on Windows).
pub const DATA_DIR_NAME: &str = "RoALT-Advanced";
/// Stamped into the file header ("ROAL") so we can tell our database from any other SQLite file.
pub const APPLICATION_ID: i64 = 0x524F_414C;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// How many safety copies of each kind are kept.
const SAFETY_KEEP: usize = 5;

/// What happened while opening; shown to the user when something noteworthy occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenReport {
    /// Where the unusable previous database was moved, if there was one.
    pub recovered_from: Option<PathBuf>,
    /// Migration versions applied during this start.
    pub migrated: Vec<u32>,
    pub schema_version: u32,
}

pub struct Database {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Database {
    /// Opens (creating if needed) the database at `path` using the real migrations.
    pub fn open(path: &Path, now_ms: i64) -> Result<(Self, OpenReport)> {
        Self::open_with(path, MIGRATIONS, now_ms)
    }

    pub(crate) fn open_with(
        path: &Path,
        migrations: &[Migration],
        now_ms: i64,
    ) -> Result<(Self, OpenReport)> {
        let dir = data_dir_of(path);
        fs::create_dir_all(&dir)?;

        let (conn, migrated, recovered_from) = match try_open(path, migrations, now_ms) {
            Ok((conn, migrated)) => (conn, migrated, None),
            Err(e) if e.is_corruption() => {
                eprintln!("[storage] database unusable ({e}); setting it aside and starting fresh");
                let moved = quarantine(path, now_ms)?;
                let (conn, migrated) = try_open(path, migrations, now_ms)?;
                (conn, migrated, Some(moved))
            }
            Err(e) => return Err(e),
        };
        let schema_version = migrations::current_version(&conn)?;
        Ok((
            Self {
                conn: Mutex::new(conn),
                path: path.to_owned(),
            },
            OpenReport {
                recovered_from,
                migrated,
                schema_version,
            },
        ))
    }

    /// Runs `f` with exclusive use of the connection. If an earlier closure panicked the lock is
    /// poisoned; the connection is still valid (an unfinished transaction is rolled back when its
    /// guard unwinds), so we carry on rather than take the whole app down.
    pub fn with<R>(&self, f: impl FnOnce(&mut Connection) -> Result<R>) -> Result<R> {
        let mut guard: MutexGuard<'_, Connection> =
            self.conn.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Flushes the write-ahead log into the main file. Called on exit so the file on disk is
    /// complete and the `-wal` file is empty.
    pub fn checkpoint(&self) -> Result<()> {
        self.with(|c| {
            c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
            Ok(())
        })
    }

    pub fn journal_mode(&self) -> Result<String> {
        self.with(|c| Ok(c.query_row("PRAGMA journal_mode", [], |r| r.get(0))?))
    }

    pub fn schema_version(&self) -> Result<u32> {
        self.with(|c| migrations::current_version(c))
    }
}

fn data_dir_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_owned)
}

/// Connection settings applied to every connection we open (also used for backup validation).
pub(crate) fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // WAL: readers do not block the writer and a crash cannot leave a half-written page set.
    // Returns the mode actually in force ("wal", or "delete" on file systems that cannot do WAL).
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    // FULL waits for the disk on every commit. Our writes are tiny and rare, so the cost is
    // negligible and a power cut cannot lose a committed change.
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // Never run SQL stored inside the database file (views/triggers calling unsafe functions).
    conn.pragma_update(None, "trusted_schema", false)?;
    // Keep the -wal file from growing without bound.
    conn.pragma_update(None, "journal_size_limit", 64 * 1024 * 1024)?;
    let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
    if fk != 1 {
        return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
    }
    Ok(())
}

/// Refuses a file that is a valid SQLite database but not ours; stamps a brand-new empty one.
pub(crate) fn check_identity(conn: &Connection) -> Result<()> {
    let id: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    if id == APPLICATION_ID {
        return Ok(());
    }
    let objects: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )?;
    if id == 0 && objects == 0 {
        conn.pragma_update(None, "application_id", APPLICATION_ID)?;
        return Ok(());
    }
    Err(StorageError::Corrupt(
        "the file is a database, but not a Nexora one".into(),
    ))
}

/// SQLite's fast structural check. Cost grows with file size; for a configuration database of a
/// few MB it takes milliseconds.
pub(crate) fn quick_check(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA quick_check")?;
    let rows: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .take(5)
        .collect::<std::result::Result<_, _>>()?;
    if rows.len() == 1 && rows[0] == "ok" {
        Ok(())
    } else {
        Err(StorageError::Corrupt(rows.join("; ")))
    }
}

fn try_open(path: &Path, migrations: &[Migration], now_ms: i64) -> Result<(Connection, Vec<u32>)> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    check_identity(&conn)?;
    quick_check(&conn)?;

    let plan = migrations::plan(&conn, migrations)?;
    if !plan.applied.is_empty() && !plan.pending.is_empty() {
        // Upgrading real data: keep a copy first, so a bad migration can never cost the user
        // their configuration. The copy lives in `safety/`.
        let from = plan.applied.last().copied().unwrap_or(0);
        let dest = safety_dir(path).join(format!("pre-migrate-v{from}-{}.db", file_stamp(now_ms)));
        super::backup::snapshot(&conn, &dest)?;
        prune_safety(&safety_dir(path), "pre-migrate-");
    }
    let migrated = migrations::migrate(&mut conn, migrations, now_ms)?;
    Ok((conn, migrated))
}

pub(crate) fn safety_dir(db_path: &Path) -> PathBuf {
    data_dir_of(db_path).join("safety")
}

/// Keeps only the newest [`SAFETY_KEEP`] files whose names start with `prefix`. Names contain a
/// sortable timestamp, so sorting by name sorts by age.
pub(crate) fn prune_safety(dir: &Path, prefix: &str) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix) && n.ends_with(".db"))
        })
        .collect();
    files.sort();
    let excess = files.len().saturating_sub(SAFETY_KEEP);
    for old in files.into_iter().take(excess) {
        if let Err(e) = fs::remove_file(&old) {
            eprintln!("[storage] could not remove old safety copy: {e}");
        }
    }
}

/// Moves a damaged database (and its `-wal` / `-shm` companions) into `quarantine/`. Nothing is
/// deleted: the user, or support, may still be able to recover data from it.
fn quarantine(path: &Path, now_ms: i64) -> Result<PathBuf> {
    let qdir = data_dir_of(path).join("quarantine");
    fs::create_dir_all(&qdir)?;
    let stem = format!("roalt-{}", file_stamp(now_ms));
    let mut target = qdir.join(format!("{stem}.db"));
    let mut n = 1;
    while target.exists() {
        target = qdir.join(format!("{stem}-{n}.db"));
        n += 1;
    }
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{suffix}", path.display()));
        if side.exists() {
            let dest = PathBuf::from(format!("{}{suffix}", target.display()));
            fs::rename(&side, &dest)?;
        }
    }
    if path.exists() {
        fs::rename(path, &target)?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::TempDir;

    fn open(dir: &TempDir) -> (Database, OpenReport) {
        Database::open(&dir.path().join(DB_FILE_NAME), 1_700_000_000_000).unwrap()
    }

    #[test]
    fn creates_missing_folders_and_a_working_database() {
        let t = TempDir::new();
        let nested = t.path().join("a").join("b").join(DB_FILE_NAME);
        let (db, report) = Database::open(&nested, 1).unwrap();
        assert!(nested.exists());
        assert_eq!(report.migrated, vec![1]);
        assert_eq!(report.schema_version, 1);
        assert!(report.recovered_from.is_none());
        assert_eq!(db.schema_version().unwrap(), 1);
    }

    #[test]
    fn connection_uses_wal_and_enforces_foreign_keys() {
        let t = TempDir::new();
        let (db, _) = open(&t);
        assert_eq!(db.journal_mode().unwrap(), "wal");
        db.with(|c| {
            let fk: i64 = c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
            let sync: i64 = c.query_row("PRAGMA synchronous", [], |r| r.get(0))?;
            let id: i64 = c.query_row("PRAGMA application_id", [], |r| r.get(0))?;
            assert_eq!((fk, sync, id), (1, 2, APPLICATION_ID)); // 2 = FULL
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn foreign_keys_are_really_enforced_on_our_connection() {
        // The v1 schema has no relationships (none were specified), so prove the setting works
        // with scratch tables on a connection configured exactly like the real one.
        let t = TempDir::new();
        let (db, _) = open(&t);
        db.with(|c| {
            c.execute_batch(
                "CREATE TABLE parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE child (id INTEGER PRIMARY KEY,
                     parent_id INTEGER NOT NULL REFERENCES parent(id) ON DELETE RESTRICT);
                 INSERT INTO parent VALUES (1);",
            )?;
            assert!(
                c.execute("INSERT INTO child (parent_id) VALUES (99)", [])
                    .is_err()
            );
            c.execute("INSERT INTO child (parent_id) VALUES (1)", [])?;
            assert!(c.execute("DELETE FROM parent WHERE id = 1", []).is_err());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn reopening_keeps_data_and_does_not_reapply_migrations() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        {
            let (db, _) = Database::open(&path, 1).unwrap();
            db.with(|c| {
                c.execute(
                    "INSERT INTO settings (key, value, created_at, updated_at) VALUES ('a.b','1',1,1)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        }
        let (db, report) = Database::open(&path, 2).unwrap();
        assert!(report.migrated.is_empty());
        assert!(report.recovered_from.is_none());
        let v: String = db
            .with(|c| {
                Ok(
                    c.query_row("SELECT value FROM settings WHERE key='a.b'", [], |r| {
                        r.get(0)
                    })?,
                )
            })
            .unwrap();
        assert_eq!(v, "1");
    }

    #[test]
    fn garbage_file_is_quarantined_not_deleted_and_a_fresh_db_is_made() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        fs::write(
            &path,
            b"this is definitely not a sqlite database, just some text",
        )
        .unwrap();
        let (db, report) = Database::open(&path, 1_700_000_000_000).unwrap();
        let moved = report.recovered_from.expect("should report the recovery");
        assert!(moved.starts_with(t.path().join("quarantine")));
        assert_eq!(
            fs::read(&moved).unwrap(),
            b"this is definitely not a sqlite database, just some text"
        );
        assert_eq!(report.migrated, vec![1]);
        assert_eq!(db.schema_version().unwrap(), 1);
    }

    #[test]
    fn damaged_pages_are_detected_and_quarantined() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        {
            let (db, _) = Database::open(&path, 1).unwrap();
            db.with(|c| {
                for i in 0..400 {
                    c.execute(
                        "INSERT INTO activity_logs (event_type, severity, message, created_at)
                         VALUES ('test.fill', 'info', ?1, 1)",
                        [format!("padding message number {i} {}", "x".repeat(200))],
                    )?;
                }
                Ok(())
            })
            .unwrap();
            db.checkpoint().unwrap();
        }
        // Keep the 100-byte header valid, destroy later pages.
        let mut bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 20_000, "test needs a multi-page file");
        for b in bytes.iter_mut().skip(8192).take(8192) {
            *b = 0xAB;
        }
        fs::write(&path, bytes).unwrap();
        let _ = fs::remove_file(format!("{}-wal", path.display()));
        let _ = fs::remove_file(format!("{}-shm", path.display()));

        let (db, report) = Database::open(&path, 2).unwrap();
        assert!(report.recovered_from.is_some(), "damage must be noticed");
        let n: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM activity_logs", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(n, 0, "the fresh database starts empty");
    }

    #[test]
    fn someone_elses_sqlite_file_is_not_adopted() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE notes (id INTEGER); INSERT INTO notes VALUES (1);")
                .unwrap();
        }
        let (_db, report) = Database::open(&path, 1).unwrap();
        let moved = report
            .recovered_from
            .expect("foreign database must be set aside");
        let c = Connection::open(&moved).unwrap();
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "the foreign data is preserved in quarantine");
    }

    #[test]
    fn newer_schema_is_refused_without_touching_the_file() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        {
            let (db, _) = Database::open(&path, 1).unwrap();
            db.with(|c| {
                c.execute(
                    "INSERT INTO schema_migrations VALUES (2, 'future', 'abc', 1)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        }
        let err = Database::open(&path, 2).err().expect("must fail");
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
        assert!(path.exists(), "the file stays where it is");
        assert!(!t.path().join("quarantine").exists());
    }

    #[test]
    fn upgrade_takes_a_safety_copy_first() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        let v1 = Migration {
            version: 1,
            name: "one",
            sql: "CREATE TABLE a (id INTEGER PRIMARY KEY, x TEXT);",
        };
        let v2 = Migration {
            version: 2,
            name: "two",
            sql: "ALTER TABLE a ADD COLUMN y TEXT;",
        };
        {
            let (db, _) = Database::open_with(&path, &[v1], 1_700_000_000_000).unwrap();
            db.with(|c| {
                c.execute("INSERT INTO a (x) VALUES ('precious')", [])?;
                Ok(())
            })
            .unwrap();
        }
        let (_db, report) = Database::open_with(&path, &[v1, v2], 1_700_000_100_000).unwrap();
        assert_eq!(report.migrated, vec![2]);
        let copy = safety_dir(&path).join("pre-migrate-v1-20231114-221500.db");
        assert!(copy.exists(), "expected {}", copy.display());
        let c = Connection::open(&copy).unwrap();
        let x: String = c.query_row("SELECT x FROM a", [], |r| r.get(0)).unwrap();
        assert_eq!(x, "precious");
        let cols: i64 = c
            .query_row("SELECT COUNT(*) FROM pragma_table_info('a')", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(cols, 2, "the copy is the pre-upgrade state");
    }

    #[test]
    fn failed_migration_leaves_the_database_openable_at_the_old_version() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        let v1 = Migration {
            version: 1,
            name: "one",
            sql: "CREATE TABLE a (id INTEGER PRIMARY KEY);",
        };
        let bad = Migration {
            version: 2,
            name: "bad",
            sql: "CREATE TABLE c (id INTEGER); THIS IS NOT SQL;",
        };
        drop(Database::open_with(&path, &[v1], 1).unwrap());
        let err = Database::open_with(&path, &[v1, bad], 2)
            .err()
            .expect("must fail");
        assert!(
            matches!(err, StorageError::MigrationFailed { version: 2, .. }),
            "{err}"
        );
        let (db, report) = Database::open_with(&path, &[v1], 3).unwrap();
        assert!(
            report.recovered_from.is_none(),
            "a failed migration is not corruption"
        );
        assert_eq!(db.schema_version().unwrap(), 1);
    }

    #[test]
    fn unusable_location_gives_an_error_not_a_panic() {
        let t = TempDir::new();
        // A file where the data folder should be: the folder cannot be created.
        let blocker = t.path().join("blocker");
        fs::write(&blocker, b"x").unwrap();
        let err = Database::open(&blocker.join("sub").join(DB_FILE_NAME), 1)
            .err()
            .expect("must fail");
        assert!(matches!(err, StorageError::Io(_)), "{err}");
    }

    #[test]
    fn old_safety_copies_are_pruned() {
        let t = TempDir::new();
        let dir = t.path().join("safety");
        fs::create_dir_all(&dir).unwrap();
        for i in 0..8 {
            fs::write(
                dir.join(format!("pre-migrate-v1-2024010{i}-000000.db")),
                b"x",
            )
            .unwrap();
        }
        fs::write(dir.join("pre-restore-20240101-000000.db"), b"x").unwrap();
        prune_safety(&dir, "pre-migrate-");
        let left = fs::read_dir(&dir).unwrap().count();
        assert_eq!(left, SAFETY_KEEP + 1);
        assert!(
            dir.join("pre-migrate-v1-20240107-000000.db").exists(),
            "newest kept"
        );
        assert!(
            !dir.join("pre-migrate-v1-20240100-000000.db").exists(),
            "oldest removed"
        );
        assert!(
            dir.join("pre-restore-20240101-000000.db").exists(),
            "other kinds untouched"
        );
    }
}
