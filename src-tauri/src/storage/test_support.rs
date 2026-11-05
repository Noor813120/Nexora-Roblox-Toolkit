//! Helpers for tests only: an isolated temporary folder per test, removed afterwards.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "roalt-adv-test-{}-{}-{n}",
            std::process::id(),
            super::clock::now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

use super::database::{DB_FILE_NAME, Database};

/// A brand-new migrated database in its own temporary folder. Keep the `TempDir` alive as long
/// as the database is used.
pub fn fresh() -> (TempDir, Database) {
    let dir = TempDir::new();
    let (db, _) = Database::open(&dir.path().join(DB_FILE_NAME), 1_700_000_000_000).expect("open");
    (dir, db)
}
