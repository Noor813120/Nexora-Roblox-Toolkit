# Security

Status: Phase 3. This document is the policy the code must meet. Most of it is not implemented yet;
each item says which phase builds it. Phase 3 added the database controls below.

## Principles

1. Never store a password or token in plaintext. (Phase 8)
2. Never bypass authentication or account security. Sign-in happens on Roblox's own page.
3. Never inject code into, or read or write the memory of, a Roblox client. No exploit executors,
   no anti-cheat bypasses, no DLL injection. This is out of scope for the whole project.
4. Never run an imported script automatically, and never run an untrusted script with unrestricted
   access to the machine. (Phase 6)
5. Never ask for or keep elevated privileges unless a single, user-confirmed action needs them.
6. The frontend is untrusted input. Every IPC command validates its arguments. (every phase)

## Current state (Phase 3)

| Control | State |
|---|---|
| Content Security Policy | Set in `tauri.conf.json`: same-origin scripts, `connect-src` limited to Tauri IPC, `frame-ancestors 'none'` |
| Capabilities | `main` window has `core:default` only |
| Shell, filesystem and HTTP plugins | Not installed |
| Stored secrets | None exist yet. The database holds ordinary configuration only; no credential storage was introduced in Phase 3. |
| Secrets in Git | `.gitignore` excludes `.env*`, key and certificate files, databases and `.roalt` files |
| IPC commands | `app_info`, `system_info` (read-only), and 20 database commands. Each is typed and validated; none accepts SQL; none accepts a file path |
| Database file | Plain SQLite in the user's local-data folder. **Not encrypted as a whole** (see below) |
| Database input | Every value is validated in Rust, and again by CHECK constraints in the schema. All SQL uses bound parameters |
| Backup and restore | Native dialogs opened by Rust; backups validated before use; restore validated, with a safety copy and automatic rollback; nothing uploaded; backups are not encrypted |
| Corrupt or foreign database | Moved to a quarantine folder, never deleted, and never silently ignored |

## Database security decisions (Phase 3)

- **No secrets in the database.** Passwords, cookies and tokens are not stored. When credentials
  arrive (Phase 8) they will be protected per field through a dedicated abstraction over Windows
  DPAPI or Credential Manager, so the database would hold only protected blobs. Nothing of that
  exists yet.
- **No whole-file encryption.** It would add SQLCipher (and a build of OpenSSL) and a key-management
  problem without protecting anything that needs protecting today.
- **Anyone who can read the user's profile folder can read `roalt.db` and any backup.** Do not put
  secrets in settings, profile configuration or activity metadata; these fields are plain text.
- **The frontend is untrusted.** It cannot send SQL, choose which file to back up to or restore from,
  or read tables directly. Names, JSON sizes, setting keys and ranges, event types and page sizes are
  checked in Rust. JSON configuration must be an object of limited size.
- **Integrity checks are not tamper protection.** The migration checksum, the application id and the
  integrity checks detect mistakes and damage, not an attacker who edits the file.
- Details: [DATABASE.md](DATABASE.md#security).

## Planned controls

- **Script permission manifests.** Each script declares what it needs (network hosts, folders,
  running time). Nothing outside the manifest is allowed. (Phase 6)
- **Trusted and untrusted labels.** Imported scripts start untrusted. Running one needs an explicit
  confirmation that shows its manifest. (Phase 6)
- **Restricted execution.** Child process with a timeout, cancellation, output limits, a working
  directory of its own and a scrubbed environment (no credentials). (Phase 6)
- **Audit log.** Every script run, permission grant and credential access is recorded. (Phase 6, 8)
- **Credential protection.** Windows DPAPI-protected keys with per-field encryption, optional master
  password, no credentials in logs or IPC responses. (Phase 8)
- **IPC validation tests.** Each command has tests for malformed and hostile input. (Phase 8)

## Reporting a problem

Open a private security advisory on the project's repository once it has one.
