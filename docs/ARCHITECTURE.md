# Architecture

Status: Phase 1. Only the skeleton exists. Sections marked *planned* describe the intended design,
not working code.

## Layers

```
React 19 + TypeScript (presentation only)
        |  typed IPC: commands (request/response) and events (status changes)
Tauri 2 shell  (window, CSP, capabilities)
        |
Rust backend   (owns processes, tasks, storage, secrets)
        |
Windows APIs   (all cfg(windows) code, only under platform/)
```

- **Rust owns** every Windows process operation, background task, database access and secret.
- **React owns** presentation and user interaction. It never sees a process handle, a file path it
  did not ask for, or a credential.

## Backend modules (`src-tauri/src/`)

| Module | Purpose | Phase | State |
|---|---|---|---|
| `commands/` | The IPC surface. Typed request/response, validation, `AppError`. | 1+ | `app_info` only |
| `error.rs` | `AppError` serialized as `{ code, message }`. | 1 | done, tested |
| `platform/` | Thin OS wrappers; the only place for `cfg(windows)`. | 1, 4 | `os()`, `arch()` |
| `storage/` | SQLite, migrations, settings, profiles, activity log. | 3 | empty |
| `launcher/` | Process discovery, launching, sessions, queue. | 4 | empty |
| `automation/` | Auto-join, recovery state machine, retries, cancellation. | 5 | empty |
| `scripts/` | Script Hub: import, external execution, Luau editing. | 6 | empty |
| `monitoring/` | RAM/CPU sampling, adaptive intervals, Resource Guard. | 7 | empty |
| `security/` | Credential protection, permission manifests, audit log. | 6, 8 | empty |

## Frontend (`src/`)

`app/` (shell), `api/` (typed IPC wrappers and generated bindings), `styles/` (tokens, Tailwind).
The planned `components/`, `features/*`, `hooks/` and `services/` folders are created when the
first thing is put in them (Phase 2), not before.

## IPC rules

1. Every command has typed request and response structs in Rust, exported to TypeScript with ts-rs
   (`npm run bindings`). The generated files are committed.
2. Every command validates its input before doing anything and returns `Result<_, AppError>`.
3. Responses contain data, never handles or internal paths.
4. Status updates use events, not polling from the UI.
5. A window may call only what its capability file allows (`src-tauri/capabilities/default.json`).

## Decisions

| Decision | Reason |
|---|---|
| One Rust crate (binary), modules inside | Simplest to build and test. Split into crates only if build times or isolation demand it. |
| ts-rs for shared types | Types come from the Rust structs, so the two sides cannot drift apart. The reference project uses the same approach. |
| State machines instead of poll loops *(planned)* | Process exits and launches become events; polling is limited to metrics at adaptive intervals. |
| `cfg(windows)` only under `platform/` | Keeps the rest of the logic testable on any OS. |
| Plain SQLite first *(planned, Phase 3)* | Avoids compiling OpenSSL for SQLCipher. Secrets are encrypted per field instead (Phase 8). Revisit if whole-database encryption is wanted. |
| No router, state library or UI kit yet | No dependency until a phase needs it. |
| Edition 2024, `panic = "abort"`, LTO, `opt-level = "s"` in release | Smaller, faster binary. |

## Not done / unknown

- The Windows build has not been run. Phase 1 was built and tested on Linux only (see
  `ROADMAP.md`).
- Whether to support several Roblox clients at once is undecided (Phase 4).
