-- Schema version 1. Timestamps are UTC milliseconds since 1970 (INTEGER).
-- The CHECK constraints repeat the Rust validation on purpose: Rust gives friendly messages,
-- the database guarantees the rules even if a bug in Rust lets bad data through.

CREATE TABLE settings (
    id          INTEGER PRIMARY KEY,
    key         TEXT    NOT NULL UNIQUE CHECK (length(key) BETWEEN 1 AND 128),
    value       TEXT    NOT NULL CHECK (length(value) <= 8192),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE profiles (
    id                 INTEGER PRIMARY KEY,
    name               TEXT    NOT NULL COLLATE NOCASE CHECK (length(trim(name)) BETWEEN 1 AND 64),
    description        TEXT    NOT NULL DEFAULT '' CHECK (length(description) <= 500),
    configuration_json TEXT    NOT NULL CHECK (json_valid(configuration_json) AND length(configuration_json) <= 65536),
    is_default         INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL
);
-- Names are unique ignoring ASCII upper/lower case ("Main" and "main" clash).
CREATE UNIQUE INDEX idx_profiles_name ON profiles (name);
-- At most one profile may be the default; the database itself refuses a second one.
CREATE UNIQUE INDEX idx_profiles_single_default ON profiles (is_default) WHERE is_default = 1;

-- Configuration only. Nothing in Phase 3 runs automation.
CREATE TABLE automation_profiles (
    id                 INTEGER PRIMARY KEY,
    name               TEXT    NOT NULL COLLATE NOCASE CHECK (length(trim(name)) BETWEEN 1 AND 64),
    configuration_json TEXT    NOT NULL CHECK (json_valid(configuration_json) AND length(configuration_json) <= 65536),
    enabled            INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_automation_profiles_name ON automation_profiles (name);

CREATE TABLE activity_logs (
    id            INTEGER PRIMARY KEY,
    event_type    TEXT    NOT NULL CHECK (length(event_type) BETWEEN 1 AND 64),
    severity      TEXT    NOT NULL CHECK (severity IN ('debug', 'info', 'warning', 'error')),
    message       TEXT    NOT NULL CHECK (length(message) BETWEEN 1 AND 2000),
    metadata_json TEXT    CHECK (metadata_json IS NULL OR (json_valid(metadata_json) AND length(metadata_json) <= 16384)),
    created_at    INTEGER NOT NULL
);
-- Newest-first listing uses the primary key (ids grow with time). These serve filtered lists and
-- the "delete everything older than" clean-up.
CREATE INDEX idx_activity_logs_created_at ON activity_logs (created_at);
CREATE INDEX idx_activity_logs_severity   ON activity_logs (severity, id);
CREATE INDEX idx_activity_logs_event_type ON activity_logs (event_type, id);
