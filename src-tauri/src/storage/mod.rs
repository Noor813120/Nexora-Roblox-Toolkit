//! SQLite storage: location, connection, migrations, settings, profiles, automation profiles,
//! activity logs, backup and restore. See `docs/DATABASE.md`.
//!
//! Nothing outside this module writes SQL. The IPC commands call the small typed functions here.

pub mod activity_logs;
pub mod automation_profiles;
pub mod backup;
pub mod clock;
pub mod database;
pub mod errors;
pub mod migrations;
pub mod profiles;
pub mod settings;
pub mod validation;

#[cfg(test)]
mod perf_tests;
#[cfg(test)]
pub mod test_support;

pub use database::Database;
