//! Errors raised by the storage layer.
//!
//! These keep the technical detail (SQLite messages, file paths) for the log. Before anything
//! reaches the frontend they are turned into an [`AppError`], which carries only a safe message.

use crate::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The caller sent something that breaks a rule (empty name, bad JSON, unknown key...).
    #[error("invalid input: {0}")]
    Invalid(String),
    /// The row asked for does not exist.
    #[error("not found")]
    NotFound,
    /// The change would break a uniqueness or consistency rule (duplicate name, default profile...).
    #[error("conflict: {0}")]
    Conflict(String),
    /// The database file is damaged or is not a RoALT Advanced database.
    #[error("database file is corrupt: {0}")]
    Corrupt(String),
    /// The file was written by a newer version of the app. Nothing is changed.
    #[error("database schema version {found} is newer than this app supports ({supported})")]
    SchemaTooNew { found: u32, supported: u32 },
    /// A recorded migration no longer matches the one built into this app.
    #[error("migration {version} was changed after it was applied")]
    MigrationMismatch { version: u32 },
    /// A migration failed and was rolled back; the database is as it was before.
    #[error("migration {version} failed: {message}")]
    MigrationFailed { version: u32, message: String },
    /// A backup file is unusable or the destination is not allowed.
    #[error("backup problem: {0}")]
    Backup(String),
    /// A file-system problem (permissions, disk full, missing folder...).
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
    /// SQLite reported an error that none of the cases above explains.
    #[error("database error: {0}")]
    Sqlite(rusqlite::Error),
}

impl From<rusqlite::Error> for StorageError {
    /// Maps SQLite's codes onto our cases so callers can react to corruption and constraint
    /// failures without parsing messages.
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode as C;
        if let rusqlite::Error::SqliteFailure(f, msg) = &e {
            let text = || msg.clone().unwrap_or_else(|| f.to_string());
            match f.code {
                C::NotADatabase | C::DatabaseCorrupt => return Self::Corrupt(text()),
                C::ConstraintViolation => return Self::Conflict(text()),
                _ => {}
            }
        }
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            return Self::NotFound;
        }
        Self::Sqlite(e)
    }
}

impl StorageError {
    /// True when the file itself cannot be trusted, so the safe response is to set it aside and
    /// start a fresh database. Newer-schema, failed-migration and permission errors are NOT in
    /// this set: the data there may be fine and must not be moved away.
    pub fn is_corruption(&self) -> bool {
        matches!(self, Self::Corrupt(_))
    }
}

impl From<StorageError> for AppError {
    fn from(e: StorageError) -> Self {
        match &e {
            StorageError::Invalid(m) => AppError::Invalid(m.clone()),
            StorageError::NotFound => AppError::NotFound,
            StorageError::Conflict(m) => AppError::Conflict(m.clone()),
            StorageError::Backup(m) => AppError::Invalid(m.clone()),
            _ => {
                // Detail goes to the log; the frontend only learns that storage failed.
                eprintln!("[storage] {e}");
                AppError::Internal
            }
        }
    }
}

/// Turns SQLite's "UNIQUE constraint failed" into a readable conflict; every other error passes
/// through the normal classification.
pub fn unique_to_conflict(e: rusqlite::Error, friendly: String) -> StorageError {
    if let rusqlite::Error::SqliteFailure(_, Some(msg)) = &e {
        if msg.starts_with("UNIQUE constraint failed") {
            return StorageError::Conflict(friendly);
        }
    }
    e.into()
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_corruption_triggers_recovery() {
        assert!(StorageError::Corrupt("x".into()).is_corruption());
        for e in [
            StorageError::NotFound,
            StorageError::SchemaTooNew {
                found: 9,
                supported: 1,
            },
            StorageError::MigrationFailed {
                version: 2,
                message: "x".into(),
            },
            StorageError::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        ] {
            assert!(!e.is_corruption(), "{e} must not be treated as corruption");
        }
    }

    #[test]
    fn user_facing_conversion_hides_internal_detail() {
        let app: AppError = StorageError::Corrupt("/home/secret/path".into()).into();
        assert_eq!(app.code(), "internal");
        assert!(!app.to_string().contains("secret"));
        let conflict: AppError = StorageError::Conflict("name taken".into()).into();
        assert_eq!(conflict.code(), "conflict");
    }

    #[test]
    fn sqlite_codes_are_classified() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(a INTEGER UNIQUE);INSERT INTO t VALUES (1);")
            .unwrap();
        let err = conn.execute("INSERT INTO t VALUES (1)", []).unwrap_err();
        assert!(matches!(StorageError::from(err), StorageError::Conflict(_)));
    }
}
