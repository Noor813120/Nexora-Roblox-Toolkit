# Database

Status: end of Phase 3. Built and tested on Linux, including runs of the real compiled app. **Never
run on Windows.** Statements about Windows behaviour below are marked as unverified.

The database stores ordinary configuration: preferences, profiles, automation configuration and an
activity history. It stores **no passwords, tokens or other secrets** (see [Security](#security)).

## In plain words

SQLite is a database that lives in one file on your computer. The app keeps its saved data in that
file, `roalt.db`. Because losing or damaging that file would lose the user's configuration, most of
the work in this phase is about *not* losing it: every change is all-or-nothing, a damaged file is set
aside instead of deleted, upgrades make a copy first, and there is a backup and restore that checks
files before trusting them.

## Location

| | |
|---|---|
| Folder | The operating system's local-data directory, from Tauri's path API (`app.path().local_data_dir()`), plus `RoALT-Advanced`. |
| Windows (intended) | `%LOCALAPPDATA%\RoALT-Advanced\roalt.db` *(not verified on Windows)* |
| Linux (verified) | `$XDG_DATA_HOME/RoALT-Advanced/roalt.db`, normally `~/.local/share/RoALT-Advanced/roalt.db` |
| Created | The folder is created if missing. |
| Hard-coded paths | None. |

`local_data_dir()` is used rather than `app_data_dir()` on purpose: on Windows `app_data_dir()` points
into the *Roaming* profile (`%APPDATA%`), which is not what was asked for and can be synchronised
between computers.

Files that can appear next to `roalt.db`:

| Name | What it is |
|---|---|
| `roalt.db-wal`, `roalt.db-shm` | SQLite's write-ahead log and its index. Normal while the app runs; the log is flushed into `roalt.db` when the app exits normally. |
| `safety/pre-migrate-v{N}-{time}.db` | Copy of the database taken before an upgrade. The newest 5 are kept. |
| `safety/pre-restore-{time}.db` | Copy of the database taken before a restore. The newest 5 are kept. |
| `quarantine/roalt-{time}.db` | A database file that was found damaged. **Never deleted automatically.** |

## Choice of library: rusqlite, not SQLx

| | rusqlite | SQLx |
|---|---|---|
| Model | Synchronous, thin wrapper over SQLite's C API | Asynchronous, multi-database toolkit with a connection pool |
| Fit for this app | One local file, one user, small queries | Designed for servers with many connections |
| Dependencies | One crate; SQLite is compiled in (`bundled`) so Windows needs no SQLite install | Pulls in an async runtime integration, a pool and (optionally) compile-time query checking against a live database |
| "No blocking work on the async runtime" | Met by running each call on a blocking thread (`spawn_blocking`) | Met by being async |
| Backup / online-restore API | Exposed (`backup` feature) | Not exposed; would need raw C calls |

rusqlite is the simpler choice that still meets every requirement. The cost is that every database
call must go through a blocking thread; that is done in one place (`commands/database.rs::run`).
Two dependencies were added: `rusqlite` (features `bundled`, `backup`) and `tauri-plugin-dialog`
(native file dialogs, used only from Rust).

## Connection and concurrency

- **One connection** behind a mutex. SQLite allows one writer at a time anyway; a single connection
  avoids pool bugs and keeps one place that owns the file.
- Commands are `async` and run the work in `tauri::async_runtime::spawn_blocking`, so the async
  runtime and the UI thread never wait on the disk.
- If code panics while holding the lock, the lock is recovered rather than poisoning the app; any
  unfinished transaction is rolled back when its guard unwinds.
- Settings applied to every connection (`database.rs::configure`):

| Setting | Value | Why |
|---|---|---|
| `journal_mode` | `WAL` | A crash cannot leave a half-written file; readers do not block the writer. |
| `synchronous` | `FULL` | Wait for the disk on each commit, so a power cut cannot lose a committed change. Writes are tiny and rare, so the cost is small (measured below). |
| `foreign_keys` | `ON` (and verified) | Enforce relationships. Opening fails if SQLite does not report it on. |
| `busy_timeout` | 5 s | Wait briefly instead of failing if the file is briefly locked. |
| `trusted_schema` | `OFF` | Never run SQL stored inside the file. |
| `journal_size_limit` | 64 MB | Keep the `-wal` file from growing without bound. |

- All statements use bound parameters; SQL text is never assembled from caller input. The one
  dynamic statement (activity-log filters) is built only from constant fragments.
- Multi-row changes (creating a default profile, trimming the log after an insert) run in one
  transaction and roll back completely on error.

## Schema (version 1)

Timestamps are **UTC milliseconds since 1970** stored as `INTEGER`. They sort and index correctly, take
8 bytes and have no time-zone or format ambiguity. The UI converts them to local time.

```
settings            id PK, key TEXT UNIQUE, value TEXT, created_at, updated_at
profiles            id PK, name TEXT (unique, ignoring ASCII case), description TEXT,
                    configuration_json TEXT, is_default 0/1, created_at, updated_at
automation_profiles id PK, name TEXT (unique, ignoring ASCII case), configuration_json TEXT,
                    enabled 0/1, created_at, updated_at
activity_logs       id PK, event_type TEXT, severity TEXT, message TEXT, metadata_json TEXT NULL,
                    created_at
schema_migrations   version PK, name, checksum, applied_at
```

Constraints (each is enforced in Rust **and** by the schema, so a bug in Rust cannot store bad data):

| Table | Rules |
|---|---|
| `settings` | `key` unique, 1-128 chars; `value` at most 8,192 chars. |
| `profiles` | `name` trimmed, 1-64 chars, unique ignoring ASCII case; `description` at most 500 chars; `configuration_json` valid JSON **object**, at most 64 KiB; `is_default` is 0 or 1; **at most one default** (a partial unique index). |
| `automation_profiles` | Same name and JSON rules; `enabled` is 0 or 1. Stores configuration only; nothing runs. |
| `activity_logs` | `event_type` 1-64 chars of `a-z 0-9 . _ -`; `severity` one of `debug`, `info`, `warning`, `error`; `message` 1-2,000 chars; `metadata_json` null or a valid JSON object up to 16 KiB. |

Indexes: `activity_logs(created_at)` (clean-up by age), `activity_logs(severity, id)` and
`activity_logs(event_type, id)` (filtered lists), plus the unique name indexes. Newest-first listing
uses the primary key. A test reads SQLite's query plans to confirm these indexes are used and that
newest-first needs no sort.

### Differences from the specification, and why

| Change | Reason |
|---|---|
| Timestamps are integers, not text | See above. The spec named the columns, not their types. |
| `severity` has four values including `debug` | The spec did not list values; `debug` is useful for housekeeping entries. |
| `PRAGMA application_id` is set to `0x524F414C` ("ROAL") | Lets the app tell its own file from any other SQLite file, in the live database and when validating a backup. |
| `schema_migrations` is created by the migration runner, not by migration 1 | A migration history table cannot be created by the migration that needs to be recorded in it. |
| **No foreign keys between the v1 tables** | The spec lists no relationships. Inventing one would be a guess that later phases would have to undo. Foreign-key *enforcement* is on, and a test proves it works on the real connection using scratch tables. Phase 4+ adds real relationships through migrations. |
| Settings keys are a registry (`settings.rs::KNOWN`) | The commands refuse unknown keys and out-of-range values, so the frontend cannot store arbitrary data. |
| Extra commands: `list_settings`, `database_status`, `backup_database`, `restore_database` | Needed for the Settings page, the status banner and backup/restore. |

## Migrations

Each schema change is a numbered SQL file built into the app (`storage/migrations/NNN_name.sql`,
listed in `migrations.rs::MIGRATIONS`). `schema_migrations` records which were applied, when, and a
checksum of the SQL.

On every start:

1. Read the history and compare it with the built-in list.
2. **Newer database than this app** (a version the app does not know): refuse to open, change nothing,
   tell the user to update the app.
3. **A recorded migration whose SQL no longer matches** (checksum differs) or **a gap** in the history:
   refuse. Shipped migrations must never be edited.
4. If an existing database has pending migrations, **copy it to `safety/` first**.
5. Apply each pending migration in its own transaction together with its history row. If one fails,
   that transaction rolls back and the database stays at the last good version; the app starts in the
   "database unavailable" state and explains why.

To add a migration: add `002_name.sql`, append it to `MIGRATIONS`, never touch an earlier entry. The
checksum is FNV-1a 64-bit of the SQL text. It catches accidental edits; it is **not** a security
feature.

## Startup and recovery

```
create folder -> open -> apply settings -> check it is our file -> quick_check -> migrations
                                  |
              damaged / not ours? -> move to quarantine/, create a fresh database, tell the user
              anything else wrong? -> do NOT move anything; start "unavailable" with a clear reason
```

| Situation | What happens |
|---|---|
| Missing folder or file | Created. |
| File is not a database, pages are damaged (`quick_check` fails), or it is a valid SQLite file that is not ours | The file and its `-wal`/`-shm` move to `quarantine/`; a new database is created; a banner and an `error` log entry say so. |
| Database is from a newer app version | Left untouched; "Update the app" message. |
| Migration fails or its checksum differs | Left at the old version; safety copy exists; "could not be upgraded" message. |
| Folder cannot be used (permission, disk full, a file in the way) | App still opens; banner and Settings explain; every database command answers `unavailable`. |

Permission, disk, newer-version and migration errors are deliberately **not** treated as corruption:
the data there may be fine, and moving it away would make things worse.

The app never silently continues with a broken database: either it works, or the UI says it does not.

## Activity history

- Real events only. The app records: `app.started`, `database.migrated`, `database.recovered`,
  `database.backup_created`, `database.restored`, `activity.cleared`, `activity.pruned`,
  `launcher.preferred_install_changed`. Nothing is pre-filled and there are no sample rows.
- `launcher.preferred_install_changed` (Info) records a change to the preferred Roblox installation.
  Its message names only the new choice, in words. Its metadata holds the old and the new choice as
  `{"from":…,"to":…}`, each one of `automatic`, `protocolHandler` or `versionFolder`, plus
  `unrecognised` as the old value when a stored value could not be read. Neither ever contains a path
  or any stored text. It is written in the same transaction as the change itself.
- **Bounded three ways:** at most **10,000** rows (older rows are dropped inside the same transaction
  as each insert; one indexed delete, no counting), entries older than the retention setting
  (default 30 days) are removed at startup, and a list call returns at most **200** rows.
- Pagination is "entries older than id N" (keyset), which stays fast and does not skip or repeat rows
  while new entries arrive.
- `clear_old_activity_logs` takes a number of days between 1 and 3,650.

## Settings

| Key | Meaning | Allowed | Default |
|---|---|---|---|
| `activity.retention_days` | Entries older than this are removed at startup. | 1-365 | 30 |
| `ui.dashboard_activity_count` | Recent entries shown on the Dashboard. | 3-25 | 8 |

A setting never saved reads as its default; `delete_setting` resets it. Saving a value identical to
the stored one writes nothing. A stored value that no longer passes the rules (hand-edited file) is
ignored in favour of the default and noted in the log.

### The preferred installation setting

The table also holds one setting that is deliberately **not** in the registry above:
`launcher.preferred_install`.

- Value: `{"v":1,"source":"protocolHandler"}` or `{"v":1,"source":"versionFolder"}`. No row means
  "automatic". It names a kind of source, never a path.
- Only `set_preferred_install` writes it. `get_setting`, `set_setting`, `delete_setting` and
  `list_settings` cannot reach it: an unregistered key is rejected and never listed.
- It is read with a bounded parser (values over 256 bytes are not parsed). A stored value that cannot
  be used is left untouched and reported. It is "invalid" when it is not something this version wrote
  (bad JSON, the wrong shape, an unknown source name, too long), and "unsupported" when it was written
  by a newer version (`v` above 1), names a source that cannot be preferred (`microsoftStore` or
  `platform`), or is a valid source saved on a system where it cannot be used. Only the user's
  explicit choice replaces it.
- No migration was needed; the schema stays at version 1.
- A backup carries it and a restore brings it back byte for byte. Because only a source kind is
  stored, a restored preference is judged against the installations on the computer it lands on: a
  missing source is reported as unavailable and is never silently replaced. (Tested with a second
  database and made-up machines, not with a second computer.)

Changing it and logging it is one step. `settings::change_raw_logged` takes a write transaction before
it reads, reads the stored value, lets the caller decide whether anything changes, writes or removes
the setting, inserts the activity entry (applying the row cap) and commits. If any step fails, none of
it is kept. If the caller decides nothing changes (the same choice again, in any equivalent spelling),
nothing is written and nothing is logged. It refuses any key that is in the registry (those must go
through `set_setting` so their range is checked), and it has one caller.

## Profiles and automation profiles

Stored and managed only. The launcher and automation engine do not exist yet.

Profile rules: unique names (ignoring ASCII case; non-ASCII letters such as `É`/`é` count as
different), configuration must be a JSON object, **at most one default**, making a profile the default
atomically un-defaults the previous one, the default cannot be un-set directly (make another profile
the default), and it cannot be deleted while other profiles exist. These rules exist so that once a
default exists, the app cannot end up in an inconsistent "no default but several profiles" state by
accident. There is no profile UI in Phase 3; the commands and their tests exist for Phase 4.

## IPC commands

All are typed (Rust structs exported to TypeScript), validated, return `AppError` on failure
(`invalid`, `not_found`, `conflict`, `unavailable`, `internal`) and run on a blocking thread. **None
accepts SQL, and none accepts a file path.**

| Group | Commands |
|---|---|
| Status | `database_status` |
| Settings | `get_setting`, `list_settings`, `set_setting`, `delete_setting` |
| Profiles | `list_profiles`, `create_profile`, `update_profile`, `delete_profile` |
| Automation profiles | `list_automation_profiles`, `create_automation_profile`, `update_automation_profile`, `delete_automation_profile` |
| Activity | `list_activity_logs`, `create_activity_log`, `clear_old_activity_logs` |
| Backup | `backup_database`, `restore_database` |

Two further commands, `get_install_selection` and `set_preferred_install` (in `commands/roblox.rs`),
read the preferred installation setting described above, and write it (together with its activity
entry) when it changes. They are described in [ARCHITECTURE.md](ARCHITECTURE.md#ipc-commands).
Neither accepts SQL or a path.

## Backup and restore

**Backup** (`Settings → Database → Create backup…`)

1. Rust opens the operating system's save dialog. The user chooses the location. The frontend never
   supplies a path.
2. `VACUUM INTO` writes a complete, consistent, standalone copy, including data still in the
   write-ahead log, to a temporary file next to the destination.
3. The temporary file is **validated** (below). Only then is it renamed to the chosen name. A failed or
   interrupted backup leaves no partial file under the final name.
4. An existing file is replaced only if the save dialog asked the user and they agreed. The live
   database file itself, and folders, are refused.
5. Nothing is uploaded anywhere.

**Validation** (used for backups and before restoring): the file is a readable SQLite database; its
`application_id` is ours; `integrity_check` is `ok`; `foreign_key_check` is empty; the migration
history is known and not from a newer app; the required tables exist.

**Restore** (`Restore from backup…`, after a confirmation step in the UI)

1. The user picks a file in the native open dialog.
2. The file is **copied** into the app's own folder (it cannot change while being read) and the copy is
   validated. Nothing has been touched yet.
3. The current database is copied to `safety/pre-restore-{time}.db`.
4. SQLite's online-backup API loads the backup into the live database in **one step**: readers and the
   writer see the old content or the new, never a mixture. No file is swapped under an open handle.
5. Settings are re-applied, older backups are upgraded by the normal migrations, and integrity is
   checked again.
6. If anything fails after step 4, the safety copy is loaded back automatically, and the user is told
   their data was put back unchanged. If even that fails, the message names the safety copy.

A restore replaces **everything** (preferences, profiles, automation configuration, activity history).
Only the single file chosen is considered: a `-wal` file sitting next to a chosen database is ignored,
which is correct for backups this app creates (they are single files).

Backups are **not encrypted**. They contain ordinary configuration only, but they are readable by
anyone who can read the file.

## Error handling

- Storage errors (`StorageError`) keep the technical detail; before reaching the frontend they become
  `AppError`, which carries a safe message (no paths, no SQLite text). Detail goes to standard error
  with a `[storage]` prefix. There is no logging framework yet; Phase 7/8 should add one.
- Nothing swallows a database error silently. The places that deliberately carry on (a startup,
  backup or restore *log entry* could not be written; a status field could not be read; the
  checkpoint on exit failed; an old safety copy could not be removed) print to the log and do not
  hide the main result. The only silent steps are best-effort deletions of the app's own temporary
  files after an operation has already finished or failed.
- Startup problems become state, not a crash: the window opens and says what is wrong.

## Security

- **No secrets are stored.** No passwords, cookies, tokens or keys exist in the database, and no
  command can ask for them. Credential storage is **not** introduced in this phase.
- **The whole file is not encrypted.** Encrypting everything would add a heavy dependency (SQLCipher,
  which compiles OpenSSL) and a key-management problem for no benefit while the file holds ordinary
  configuration. This settles the question left open in the roadmap.
- **Future sensitive values** (Phase 8) are to be protected field by field through a dedicated
  security abstraction (`src-tauri/src/security/`) backed by Windows DPAPI or Credential Manager, so
  the database would hold only protected blobs. Nothing of that exists yet.
- The frontend cannot run SQL, choose file paths, or read other tables: every command is a fixed,
  validated operation. Backup and restore dialogs are opened by Rust.
- `.gitignore` excludes `*.db`, `*.db-wal`, `*.db-shm` and `*.roalt`; no database file is committed.
- Honest limits: a person with access to the user's profile can read or change `roalt.db`; the
  checksums and application id detect mistakes, not tampering.

## Testing

Rust (storage is covered by 86 of the 125 tests; each uses its own temporary folder and real SQLite
files, never shared state): initialisation and folder creation; WAL/foreign-key/synchronous settings;
foreign keys enforced on the real connection; migrations (fresh, repeat, upgrade with data, rollback of
a failing migration, newer database, edited migration, gap, malformed list); schema and indexes; reopening;
settings, profiles, automation profiles and activity logs (CRUD, validation, uniqueness, default rules,
cap, pagination, filters, retention, clean-up, injection attempts, schema-level rejection of bad rows,
rollback of a multi-step change); backup (standalone file, WAL contents captured, overwrite protection,
refusal of bad destinations, no partial file, consistency while another thread writes); validation
(not a database, foreign database, truncated, damaged, newer, tampered, incomplete); restore (state
brought back, reopen, brand-new install, invalid files change nothing, older backup upgraded, newer
refused, failure mid-way rolls back, safety copies pruned); corruption handling (garbage file, damaged
pages, someone else's SQLite file); startup bookkeeping and failure states.

Phase 4 Milestone 2 added tests for the atomic change (rejected log entries, aborted inserts and
failed row-cap pruning are each tested at the activity-log helper level, with a setting written in the
same transaction; the first two are also tested through the settings change itself), the choice rules,
the ordering of candidates over every permutation of a mixed set, the exact stored text and activity
entries, backup and restore of the preference (including a value from a newer version, byte for
byte), and concurrent writers on the one connection (each entry's "from" matches the previous "to").
At the time of writing the whole Rust suite is 251 passing tests, with 1 ignored, run on Windows.

The tests were also **checked for real strength by deliberately breaking the code** (skipping
validation, allowing silent overwrite, removing the restore rollback, removing the log cap, always
rewriting settings, turning foreign keys off, not clearing the old default): each break made tests fail.

Frontend (115 tests in total): the typed service (every command name and argument shape), the hooks,
the Dashboard activity list, the Logs page (loading, empty, error, offline, filter, paging, stale
replies, clean-up), the Settings page (saving, validation, reset, errors, unavailable database), the
Database card (status, backup, restore confirmation, cancel, errors, double-click protection) and the
status banner. Five frontend mutations were checked the same way.

Beyond unit tests, the **real compiled app** was run under a virtual display on Linux against a real
database: first start (migration and `app.started` shown on the Dashboard and Logs page), a setting
saved and still present after restarting the app, a backup written through the native save dialog and
verified independently with Python's `sqlite3` (integrity `ok`, application id, tables, saved value),
a restore through the native open dialog (value and Preferences panel returned to the backed-up state,
safety copy kept, `database.restored` logged), the overwrite prompt appearing for an existing file, a
damaged database quarantined with a banner, and an unusable data folder producing the "unavailable"
state with backup and restore disabled.

## Known limits and unverified items

- **Windows is untested.** Unverified: that `local_data_dir()` resolves to `%LOCALAPPDATA%`; WAL and
  `rename`-over-existing behaviour with antivirus or sync tools holding the file; and that the Windows
  save dialog asks before replacing a file (assumed from Windows' standard behaviour; seen on Linux).
  If a dialog ever did not ask, an existing file would be replaced without a prompt.
- Name uniqueness ignores case for ASCII only (SQLite's `NOCASE`).
- The 10,000-row cap relies on ids being assigned in order and old rows only ever being removed from
  the old end, which the code guarantees; a hand-edited database could break that assumption.
- Backups made by this app are single files. A backup file with a stray `-wal` beside it is not
  supported.
- A crash (not a normal exit) leaves a `-wal` file; SQLite recovers it on the next start (the
  persistence test restarts the app after a forced stop).
- There is no logging framework yet (messages go to standard error).
- The preferred-installation row is changed only through its command, but, like everything else in the
  file, a person with access to `roalt.db` can edit it by hand. An unreadable value is reported and
  kept, not repaired.
