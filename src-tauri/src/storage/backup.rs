//! Backup and restore.
//!
//! **Backup** uses SQLite's `VACUUM INTO`, which writes a complete, consistent copy of the
//! database as one standalone file, including everything still sitting in the write-ahead log. The
//! copy is written to a temporary name, validated, and only then moved to the name the user chose,
//! so a failed or interrupted backup never leaves a half-written file under the final name and
//! never replaces an existing file unless the user agreed to that.
//!
//! **Restore** never swaps files underneath an open connection. Instead it: copies the chosen file
//! into the app's own folder (so it cannot change while we read it), validates the copy, saves a
//! safety copy of the current database, then loads the backup into the live database with SQLite's
//! online-backup API, which replaces everything in a single step. If anything goes wrong after
//! that point, the safety copy is loaded back.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, backup::StepResult};
use serde::Serialize;
use ts_rs::TS;

use super::clock::file_stamp;
use super::database::{APPLICATION_ID, Database, configure, prune_safety, safety_dir};
use super::errors::{Result, StorageError};
use super::migrations::{self, MIGRATIONS, Migration};

const REQUIRED_TABLES: [&str; 4] = [
    "settings",
    "profiles",
    "automation_profiles",
    "activity_logs",
];

/// Facts about a validated database file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupInfo {
    pub schema_version: u32,
    pub size_bytes: u64,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreInfo {
    pub schema_version: u32,
    /// Migrations that had to be applied because the backup came from an older version.
    pub migrated_versions: Vec<u32>,
    /// File name (not path) of the copy of the previous database, kept in the app's data folder.
    pub safety_copy: String,
}

fn backup_err<T>(msg: impl Into<String>) -> Result<T> {
    Err(StorageError::Backup(msg.into()))
}

/// Writes a consistent copy of `conn`'s database to `dest` (which must not exist).
pub(crate) fn snapshot(conn: &Connection, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if dest.exists() {
        return Err(StorageError::Backup(
            "the backup destination already exists".into(),
        ));
    }
    // `VACUUM INTO` takes the file name as a bound value, so no path is spliced into SQL text.
    conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
    Ok(())
}

/// Opens `path` read-only and checks everything we can before trusting it: it is a SQLite file,
/// it is ours, its pages are intact, its relationships are intact, its migration history is
/// known, and the tables we need exist.
pub fn validate_file(path: &Path, migrations: &[Migration]) -> Result<BackupInfo> {
    let meta =
        fs::metadata(path).map_err(|_| StorageError::Backup("the file cannot be read".into()))?;
    if !meta.is_file() {
        return backup_err("that is not a file");
    }
    if meta.len() == 0 {
        return backup_err("the file is empty");
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| StorageError::Backup("the file cannot be opened as a database".into()))?;

    let id: i64 = conn
        .query_row("PRAGMA application_id", [], |r| r.get(0))
        .map_err(|_| StorageError::Backup("the file is not a SQLite database".into()))?;
    if id != APPLICATION_ID {
        return backup_err("the file is not a Nexora backup");
    }

    let integrity: Vec<String> = {
        let mut s = conn
            .prepare("PRAGMA integrity_check")
            .map_err(|_| StorageError::Backup("the backup is damaged".into()))?;
        s.query_map([], |r| r.get(0))
            .and_then(|rows| rows.take(5).collect::<std::result::Result<_, _>>())
            .map_err(|_| StorageError::Backup("the backup is damaged".into()))?
    };
    if integrity != ["ok"] {
        return backup_err("the backup failed its integrity check and is damaged");
    }
    let fk_problems: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .map_err(|_| StorageError::Backup("the backup is damaged".into()))?;
    if fk_problems != 0 {
        return backup_err("the backup contains broken relationships");
    }

    let has_history: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
        [],
        |r| r.get(0),
    )?;
    if has_history == 0 {
        return backup_err("the backup has no migration history");
    }
    match migrations::plan(&conn, migrations) {
        Ok(_) => {}
        Err(StorageError::SchemaTooNew { .. }) => {
            return backup_err("the backup was made by a newer version of Nexora");
        }
        Err(_) => return backup_err("the backup's migration history does not match this app"),
    }
    let version = migrations::current_version(&conn)?;
    if version >= 1 {
        for table in REQUIRED_TABLES {
            let found: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )?;
            if found == 0 {
                return backup_err(format!("the backup is missing the {table} table"));
            }
        }
    }
    Ok(BackupInfo {
        schema_version: version,
        size_bytes: meta.len(),
    })
}

/// Removes a file when dropped, so temporary files disappear on every exit path.
struct Cleanup(Vec<PathBuf>);
impl Drop for Cleanup {
    fn drop(&mut self) {
        for p in &self.0 {
            for suffix in ["", "-wal", "-shm", "-journal"] {
                let _ = fs::remove_file(format!("{}{suffix}", p.display()));
            }
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

impl Database {
    /// Writes a validated backup to `dest`. An existing file is replaced only when
    /// `replace_existing` is true (the caller sets that after the user confirmed in the OS
    /// save dialog).
    pub fn backup_to(&self, dest: &Path, replace_existing: bool) -> Result<BackupInfo> {
        self.backup_to_with(dest, replace_existing, MIGRATIONS)
    }

    pub(crate) fn backup_to_with(
        &self,
        dest: &Path,
        replace_existing: bool,
        migrations: &[Migration],
    ) -> Result<BackupInfo> {
        if same_file(dest, self.path()) {
            return backup_err("the backup cannot be written over the live database");
        }
        if dest.is_dir() {
            return backup_err("the chosen location is a folder, not a file");
        }
        if dest.exists() && !replace_existing {
            return backup_err("a file with that name already exists; it was not overwritten");
        }
        let name = dest
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| StorageError::Backup("the file name is not valid".into()))?;
        // Same folder as the destination, so the final move is a rename on one volume.
        let temp = dest.with_file_name(format!(".{name}.roalt-tmp"));
        let _cleanup = Cleanup(vec![temp.clone()]);
        let _ = fs::remove_file(&temp); // a leftover from an earlier crash

        self.with(|c| snapshot(c, &temp))?;
        let info = validate_file(&temp, migrations)?;
        // Validation may have created -wal/-shm next to the temp file; they are removed when the
        // validating connection closed. The final file must stand alone:
        fs::rename(&temp, dest)?;
        Ok(info)
    }

    /// Replaces the contents of the live database with the backup at `src`.
    pub fn restore_from(&self, src: &Path, now_ms: i64) -> Result<RestoreInfo> {
        self.restore_from_with(src, MIGRATIONS, now_ms)
    }

    pub(crate) fn restore_from_with(
        &self,
        src: &Path,
        migrations: &[Migration],
        now_ms: i64,
    ) -> Result<RestoreInfo> {
        if same_file(src, self.path()) {
            return backup_err("choose a backup file, not the live database");
        }
        let safety = safety_dir(self.path());
        fs::create_dir_all(&safety)?;
        let stamp = file_stamp(now_ms);

        // 1. Private copy, so the file cannot change under us and can be opened in our folder.
        let incoming = safety.join(format!("incoming-{stamp}.tmp"));
        let _cleanup = Cleanup(vec![incoming.clone()]);
        fs::copy(src, &incoming)
            .map_err(|_| StorageError::Backup("the backup file could not be read".into()))?;
        // 2. Validate before touching anything.
        validate_file(&incoming, migrations)?;

        // 3. Safety copy of what is there now.
        let safety_name = format!("pre-restore-{stamp}.db");
        let safety_path = safety.join(&safety_name);
        let _ = fs::remove_file(&safety_path);
        self.with(|c| snapshot(c, &safety_path))?;

        // 4. Replace, then bring the result up to date; undo on any failure.
        let result = self.with(|conn| {
            load_into(conn, &incoming)?;
            configure(conn)?;
            let migrated = migrations::migrate(conn, migrations, now_ms)?;
            super::database::quick_check(conn)?;
            let broken: i64 =
                conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            if broken != 0 {
                return Err(StorageError::Corrupt(
                    "broken relationships after restore".into(),
                ));
            }
            Ok((migrated, migrations::current_version(conn)?))
        });
        match result {
            Ok((migrated_versions, schema_version)) => {
                prune_safety(&safety, "pre-restore-");
                Ok(RestoreInfo {
                    schema_version,
                    migrated_versions,
                    safety_copy: safety_name,
                })
            }
            Err(e) => {
                eprintln!("[storage] restore failed ({e}); putting the previous database back");
                let undo = self.with(|conn| {
                    load_into(conn, &safety_path)?;
                    configure(conn)
                });
                if let Err(undo_err) = undo {
                    eprintln!("[storage] could not undo the restore: {undo_err}");
                    return Err(StorageError::Backup(format!(
                        "the restore failed and the previous database could not be put back automatically; \
                         a copy is kept as {safety_name} in the app's data folder"
                    )));
                }
                match e {
                    StorageError::Backup(_) | StorageError::Invalid(_) => Err(e),
                    _ => Err(StorageError::Backup(
                        "the restore failed; your previous data was put back unchanged".into(),
                    )),
                }
            }
        }
    }
}

/// Copies every page of `from_file` over the connection's database in one step. Readers and the
/// writer see either the old content or the new content, never a mixture.
fn load_into(conn: &mut Connection, from_file: &Path) -> Result<()> {
    let src = Connection::open_with_flags(from_file, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let bk = rusqlite::backup::Backup::new(&src, conn)?;
    match bk.step(-1)? {
        StepResult::Done => Ok(()),
        other => Err(StorageError::Backup(format!(
            "the database was busy during restore ({other:?}); nothing was changed"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::activity_logs::{self, NewActivityLog, Severity};
    use crate::storage::database::DB_FILE_NAME;
    use crate::storage::profiles::{self, ProfileInput};
    use crate::storage::test_support::{TempDir, fresh};
    use crate::storage::{automation_profiles as auto, settings};
    use std::sync::Arc;

    const NOW: i64 = 1_700_000_000_000;

    /// Puts one of everything into the database.
    fn fill(db: &Database, tag: &str) {
        db.with(|c| {
            settings::set(c, settings::RETENTION_DAYS, "45", 1)?;
            profiles::create(
                c,
                &ProfileInput {
                    name: format!("profile-{tag}"),
                    description: String::new(),
                    configuration_json: r#"{"a":1}"#.into(),
                    is_default: true,
                },
                2,
            )?;
            auto::create(
                c,
                &auto::AutomationProfileInput {
                    name: format!("auto-{tag}"),
                    configuration_json: "{}".into(),
                    enabled: true,
                },
                3,
            )?;
            activity_logs::create(
                c,
                &NewActivityLog {
                    event_type: "test.fill".into(),
                    severity: Severity::Info,
                    message: format!("log-{tag}"),
                    metadata_json: None,
                },
                4,
            )?;
            Ok(())
        })
        .unwrap();
    }

    /// (retention setting, profile names, automation names, log messages) as stored.
    fn snapshot_of(db: &Database) -> (String, Vec<String>, Vec<String>, Vec<String>) {
        db.with(|c| {
            Ok((
                settings::entry(c, settings::RETENTION_DAYS)?.value,
                profiles::list(c)?.into_iter().map(|p| p.name).collect(),
                auto::list(c)?.into_iter().map(|p| p.name).collect(),
                activity_logs::list(c, &Default::default())?
                    .items
                    .into_iter()
                    .map(|l| l.message)
                    .collect(),
            ))
        })
        .unwrap()
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    // ---------------- backup ----------------

    #[test]
    fn backup_is_a_complete_standalone_file() {
        let (_t, db) = fresh();
        fill(&db, "one");
        // Deliberately no checkpoint: the data may still be in the -wal file. VACUUM INTO must
        // capture it anyway.
        let out = TempDir::new();
        let dest = out.path().join("backup.db");
        let info = db.backup_to(&dest, false).unwrap();
        assert_eq!(info.schema_version, 1);
        assert!(info.size_bytes > 0);
        assert_eq!(
            files_in(out.path()),
            ["backup.db"],
            "no temp, -wal or -shm files left behind"
        );

        // Move the single file somewhere else entirely and open it as a database.
        let elsewhere = TempDir::new();
        let moved = elsewhere.path().join("moved.db");
        fs::copy(&dest, &moved).unwrap();
        let (copy, report) = Database::open(&moved, NOW).unwrap();
        assert!(report.migrated.is_empty() && report.recovered_from.is_none());
        assert_eq!(snapshot_of(&copy), snapshot_of(&db));
    }

    #[test]
    fn backup_does_not_disturb_the_live_database() {
        let (_t, db) = fresh();
        fill(&db, "one");
        let before = snapshot_of(&db);
        let out = TempDir::new();
        db.backup_to(&out.path().join("b.db"), false).unwrap();
        assert_eq!(snapshot_of(&db), before);
        assert_eq!(db.journal_mode().unwrap(), "wal");
        db.with(|c| settings::set(c, settings::RETENTION_DAYS, "10", 9).map(|_| ()))
            .unwrap();
    }

    #[test]
    fn backup_taken_during_writes_is_consistent() {
        let (_t, db) = fresh();
        let db = Arc::new(db);
        let writer = {
            let db = Arc::clone(&db);
            std::thread::spawn(move || {
                for i in 0..300 {
                    db.with(|c| {
                        activity_logs::create(
                            c,
                            &NewActivityLog {
                                event_type: "test.write".into(),
                                severity: Severity::Info,
                                message: format!("w{i}"),
                                metadata_json: None,
                            },
                            i,
                        )
                        .map(|_| ())
                    })
                    .unwrap();
                }
            })
        };
        let out = TempDir::new();
        for n in 0..5 {
            let dest = out.path().join(format!("b{n}.db"));
            // validate_file runs inside backup_to: integrity_check, foreign_key_check, history.
            db.backup_to(&dest, false).unwrap();
            validate_file(&dest, MIGRATIONS).unwrap();
        }
        writer.join().unwrap();
    }

    #[test]
    fn existing_files_are_not_overwritten_unless_confirmed() {
        let (_t, db) = fresh();
        fill(&db, "one");
        let out = TempDir::new();
        let dest = out.path().join("precious.db");
        fs::write(&dest, b"somebody's other file").unwrap();

        let err = db.backup_to(&dest, false).unwrap_err();
        assert!(
            matches!(err, StorageError::Backup(ref m) if m.contains("already exists")),
            "{err}"
        );
        assert_eq!(fs::read(&dest).unwrap(), b"somebody's other file");
        assert_eq!(files_in(out.path()), ["precious.db"]);

        db.backup_to(&dest, true).unwrap();
        validate_file(&dest, MIGRATIONS).unwrap();
    }

    #[test]
    fn backup_refuses_bad_destinations() {
        let (t, db) = fresh();
        // The live database itself.
        let live = t.path().join(DB_FILE_NAME);
        assert!(matches!(
            db.backup_to(&live, true),
            Err(StorageError::Backup(_))
        ));
        assert!(matches!(
            db.backup_to(&live, false),
            Err(StorageError::Backup(_))
        ));
        validate_file(&live.with_extension("none"), MIGRATIONS).unwrap_err(); // a missing file
        // A folder.
        let out = TempDir::new();
        assert!(matches!(
            db.backup_to(out.path(), true),
            Err(StorageError::Backup(_))
        ));
        // A place that cannot exist (a file where a folder is needed).
        let blocker = out.path().join("blocker");
        fs::write(&blocker, b"x").unwrap();
        assert!(
            db.backup_to(&blocker.join("sub").join("b.db"), false)
                .is_err()
        );
        // The live database still works after all those refusals.
        fill(&db, "after");
    }

    #[test]
    fn failed_backup_leaves_no_partial_file() {
        let (_t, db) = fresh();
        let out = TempDir::new();
        let blocker = out.path().join("blocker");
        fs::write(&blocker, b"x").unwrap();
        let _ = db.backup_to(&blocker.join("b.db"), false);
        assert_eq!(files_in(out.path()), ["blocker"]);
    }

    // ---------------- validation ----------------

    fn good_backup(db: &Database, dir: &TempDir) -> PathBuf {
        let p = dir.path().join("good.db");
        db.backup_to(&p, false).unwrap();
        p
    }

    #[test]
    fn validation_accepts_a_real_backup() {
        let (_t, db) = fresh();
        fill(&db, "x");
        let d = TempDir::new();
        let info = validate_file(&good_backup(&db, &d), MIGRATIONS).unwrap();
        assert_eq!(info.schema_version, 1);
    }

    #[test]
    fn validation_rejects_files_that_are_not_backups() {
        let d = TempDir::new();
        let check = |name: &str, bytes: &[u8]| {
            let p = d.path().join(name);
            fs::write(&p, bytes).unwrap();
            validate_file(&p, MIGRATIONS).unwrap_err()
        };
        assert!(
            matches!(check("empty.db", b""), StorageError::Backup(ref m) if m.contains("empty"))
        );
        assert!(matches!(
            check("text.db", b"hello, I am a text file"),
            StorageError::Backup(_)
        ));
        assert!(matches!(
            validate_file(d.path(), MIGRATIONS).unwrap_err(),
            StorageError::Backup(_)
        ));

        // A genuine SQLite database that belongs to something else.
        let other = d.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE t(a); INSERT INTO t VALUES (1);")
            .unwrap();
        let err = validate_file(&other, MIGRATIONS).unwrap_err();
        assert!(
            matches!(err, StorageError::Backup(ref m) if m.contains("not a Nexora")),
            "{err}"
        );
    }

    #[test]
    fn validation_rejects_damaged_backups() {
        let (_t, db) = fresh();
        db.with(|c| {
            for i in 0..400 {
                activity_logs::create(
                    c,
                    &NewActivityLog {
                        event_type: "test.fill".into(),
                        severity: Severity::Info,
                        message: format!("padding {i} {}", "x".repeat(200)),
                        metadata_json: None,
                    },
                    i,
                )?;
            }
            Ok(())
        })
        .unwrap();
        let d = TempDir::new();
        let good = good_backup(&db, &d);
        let mut bytes = fs::read(&good).unwrap();
        assert!(bytes.len() > 20_000);
        // Truncated, and with a damaged middle.
        let truncated = d.path().join("truncated.db");
        fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();
        for b in bytes.iter_mut().skip(8192).take(8192) {
            *b ^= 0xFF;
        }
        let damaged = d.path().join("damaged.db");
        fs::write(&damaged, &bytes).unwrap();
        for p in [truncated, damaged] {
            assert!(
                matches!(validate_file(&p, MIGRATIONS), Err(StorageError::Backup(_))),
                "{} should be rejected",
                p.display()
            );
        }
    }

    #[test]
    fn validation_rejects_newer_tampered_or_incomplete_histories() {
        let (_t, db) = fresh();
        let d = TempDir::new();

        let newer = good_backup(&db, &d);
        Connection::open(&newer)
            .unwrap()
            .execute(
                "INSERT INTO schema_migrations VALUES (2,'future','x',1)",
                [],
            )
            .unwrap();
        let err = validate_file(&newer, MIGRATIONS).unwrap_err();
        assert!(
            matches!(err, StorageError::Backup(ref m) if m.contains("newer version")),
            "{err}"
        );

        let tampered = d.path().join("tampered.db");
        db.backup_to(&tampered, false).unwrap();
        Connection::open(&tampered)
            .unwrap()
            .execute(
                "UPDATE schema_migrations SET checksum = 'deadbeef' WHERE version = 1",
                [],
            )
            .unwrap();
        assert!(matches!(
            validate_file(&tampered, MIGRATIONS),
            Err(StorageError::Backup(_))
        ));

        // Ours by id, but with the real tables missing.
        let hollow = d.path().join("hollow.db");
        {
            let c = Connection::open(&hollow).unwrap();
            c.pragma_update(None, "application_id", APPLICATION_ID)
                .unwrap();
            migrations::migrate(&mut Connection::open_in_memory().unwrap(), MIGRATIONS, 1).unwrap();
            c.execute_batch(&format!(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL);
                 INSERT INTO schema_migrations VALUES (1,'initial','{}',1);",
                migrations::checksum(MIGRATIONS[0].sql)
            ))
            .unwrap();
        }
        let err = validate_file(&hollow, MIGRATIONS).unwrap_err();
        assert!(
            matches!(err, StorageError::Backup(ref m) if m.contains("missing")),
            "{err}"
        );
    }

    // ---------------- restore ----------------

    #[test]
    fn restore_brings_back_the_backed_up_state() {
        let (t, db) = fresh();
        fill(&db, "original");
        let d = TempDir::new();
        let backup = good_backup(&db, &d);
        let backup_bytes = fs::read(&backup).unwrap();
        let original = snapshot_of(&db);

        // Change everything after the backup.
        db.with(|c| {
            settings::set(c, settings::RETENTION_DAYS, "7", 10)?;
            for p in profiles::list(c)? {
                // The profile is the default; add another default first, then remove it.
                let other = profiles::create(
                    c,
                    &ProfileInput {
                        name: "later".into(),
                        description: String::new(),
                        configuration_json: "{}".into(),
                        is_default: true,
                    },
                    11,
                )?;
                profiles::delete(c, p.id)?;
                let _ = other;
            }
            Ok(())
        })
        .unwrap();
        fill(&db, "later");
        assert_ne!(snapshot_of(&db), original);
        let before_restore = snapshot_of(&db);

        let info = db.restore_from(&backup, NOW).unwrap();
        assert_eq!(info.schema_version, 1);
        assert!(info.migrated_versions.is_empty());
        assert_eq!(
            snapshot_of(&db),
            original,
            "restored state equals the backed-up state"
        );

        // The database is fully usable afterwards, with the same guarantees.
        assert_eq!(db.journal_mode().unwrap(), "wal");
        db.with(|c| {
            let fk: i64 = c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
            assert_eq!(fk, 1);
            settings::set(c, settings::RETENTION_DAYS, "60", 20)?;
            Ok(())
        })
        .unwrap();

        // The previous state was kept, and loads as a database of its own.
        let safety = safety_dir(db.path()).join(&info.safety_copy);
        assert!(safety.exists());
        let (kept, _) = Database::open(&safety, NOW).unwrap();
        assert_eq!(snapshot_of(&kept), before_restore);

        // The backup file itself was not modified, and no temporary copy is left in the app folder.
        assert_eq!(fs::read(&backup).unwrap(), backup_bytes);
        let leftovers: Vec<String> = files_in(&safety_dir(db.path()))
            .into_iter()
            .filter(|n| n.starts_with("incoming"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        drop(t);
    }

    #[test]
    fn restored_data_survives_closing_and_reopening() {
        let t = TempDir::new();
        let path = t.path().join(DB_FILE_NAME);
        let d = TempDir::new();
        let backup;
        {
            let (db, _) = Database::open(&path, 1).unwrap();
            fill(&db, "keep");
            backup = good_backup(&db, &d);
            db.with(|c| settings::set(c, settings::RETENTION_DAYS, "5", 2).map(|_| ()))
                .unwrap();
            db.restore_from(&backup, NOW).unwrap();
        }
        let (db, report) = Database::open(&path, 3).unwrap();
        assert!(report.recovered_from.is_none());
        assert_eq!(snapshot_of(&db).0, "45");
    }

    #[test]
    fn restore_into_a_brand_new_installation() {
        // Disaster recovery: the machine is new, the user restores the backup they kept.
        let (_t1, old) = fresh();
        fill(&old, "precious");
        let d = TempDir::new();
        let backup = good_backup(&old, &d);
        let (_t2, new_install) = fresh();
        assert!(snapshot_of(&new_install).1.is_empty());
        new_install.restore_from(&backup, NOW).unwrap();
        assert_eq!(snapshot_of(&new_install), snapshot_of(&old));
    }

    #[test]
    fn invalid_restore_files_change_nothing() {
        let (t, db) = fresh();
        fill(&db, "live");
        let before = snapshot_of(&db);
        let d = TempDir::new();
        let junk = d.path().join("junk.db");
        fs::write(&junk, b"definitely not a database").unwrap();
        let foreign = d.path().join("foreign.db");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE t(a);")
            .unwrap();
        let good = good_backup(&db, &d);
        let mut bytes = fs::read(&good).unwrap();
        let cut = bytes.len() / 3;
        bytes.truncate(cut);
        let truncated = d.path().join("truncated.db");
        fs::write(&truncated, bytes).unwrap();

        for bad in [
            junk,
            foreign,
            truncated,
            d.path().join("missing.db"),
            d.path().to_owned(),
            t.path().join(DB_FILE_NAME), // the live file itself
        ] {
            let err = db.restore_from(&bad, NOW).unwrap_err();
            assert!(
                matches!(err, StorageError::Backup(_) | StorageError::Io(_)),
                "{}: {err}",
                bad.display()
            );
            assert_eq!(
                snapshot_of(&db),
                before,
                "{} must not change the database",
                bad.display()
            );
        }
        // Rejected before any safety copy was needed.
        assert!(
            !safety_dir(db.path()).exists()
                || files_in(&safety_dir(db.path()))
                    .iter()
                    .all(|n| !n.starts_with("pre-restore")),
        );
        assert_eq!(db.journal_mode().unwrap(), "wal");
    }

    fn v2() -> Migration {
        Migration {
            version: 2,
            name: "add note column",
            sql: "ALTER TABLE settings ADD COLUMN note TEXT;",
        }
    }

    #[test]
    fn restoring_an_older_backup_upgrades_it() {
        let d = TempDir::new();
        let (_t1, old) = fresh();
        fill(&old, "old-app");
        let backup = good_backup(&old, &d); // made by "version 1" of the app

        // The running app is "version 2".
        let t2 = TempDir::new();
        let (db, _) =
            Database::open_with(&t2.path().join(DB_FILE_NAME), &[MIGRATIONS[0], v2()], NOW)
                .unwrap();
        let info = db
            .restore_from_with(&backup, &[MIGRATIONS[0], v2()], NOW)
            .unwrap();
        assert_eq!(info.migrated_versions, vec![2]);
        assert_eq!(info.schema_version, 2);
        assert_eq!(snapshot_of(&db).2, ["auto-old-app"]);
        db.with(|c| {
            c.execute(
                "UPDATE settings SET note = 'works' WHERE key = ?1",
                [settings::RETENTION_DAYS],
            )?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_backup_from_a_newer_app_is_refused() {
        let d = TempDir::new();
        let t_new = TempDir::new();
        let (new_db, _) = Database::open_with(
            &t_new.path().join(DB_FILE_NAME),
            &[MIGRATIONS[0], v2()],
            NOW,
        )
        .unwrap();
        let backup = d.path().join("new.db");
        new_db
            .backup_to_with(&backup, false, &[MIGRATIONS[0], v2()])
            .unwrap();

        let (_t, db) = fresh();
        fill(&db, "mine");
        let before = snapshot_of(&db);
        let err = db.restore_from(&backup, NOW).unwrap_err();
        assert!(
            matches!(err, StorageError::Backup(ref m) if m.contains("newer version")),
            "{err}"
        );
        assert_eq!(snapshot_of(&db), before);
    }

    #[test]
    fn a_restore_that_fails_midway_puts_the_old_data_back() {
        let d = TempDir::new();
        let (_t1, source) = fresh();
        fill(&source, "from-backup");
        let backup = good_backup(&source, &d);

        let t2 = TempDir::new();
        let (db, _) = Database::open(&t2.path().join(DB_FILE_NAME), NOW).unwrap();
        fill(&db, "live");
        let before = snapshot_of(&db);

        // The backup is valid, but upgrading it fails (a deliberately broken migration 2).
        let broken = Migration {
            version: 2,
            name: "broken",
            sql: "ALTER TABLE settings ADD COLUMN ok TEXT; THIS IS NOT SQL;",
        };
        let err = db
            .restore_from_with(&backup, &[MIGRATIONS[0], broken], NOW)
            .unwrap_err();
        assert!(
            matches!(err, StorageError::Backup(ref m) if m.contains("put back unchanged")),
            "{err}"
        );
        assert_eq!(snapshot_of(&db), before, "live data is exactly as before");
        assert_eq!(db.schema_version().unwrap(), 1);
        assert_eq!(db.journal_mode().unwrap(), "wal");
        // Still writable.
        db.with(|c| settings::set(c, settings::RETENTION_DAYS, "11", 5).map(|_| ()))
            .unwrap();
        // And a normal restore works afterwards.
        db.restore_from(&backup, NOW + 1000).unwrap();
        assert_eq!(snapshot_of(&db).1, ["profile-from-backup"]);
    }

    #[test]
    fn safety_copies_are_pruned_across_many_restores() {
        let (_t, db) = fresh();
        fill(&db, "x");
        let d = TempDir::new();
        let backup = good_backup(&db, &d);
        for i in 0..8 {
            db.restore_from(&backup, NOW + i * 1000).unwrap();
        }
        let copies = files_in(&safety_dir(db.path()))
            .into_iter()
            .filter(|n| n.starts_with("pre-restore-"))
            .count();
        assert_eq!(copies, 5);
    }
}
