# Audit of the reference repository

Reference: <https://github.com/inspectsoftware/RoALT> (MIT, "Copyright (c) 2026 inspectsoftware").
Audited at the latest commit on 2026-11-04 ("License RoALT under MIT"), version 0.9.2, from a
shallow clone. The reference was only read; nothing in it was changed.

## What was read, and what was not

Read in full or in large part: `README.md`, `LICENSE`, `package.json`, `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json`, `docs/MILESTONES.md` (headings, M1-M4), `docs/DESIGN.md` (headings,
sections 1-2, parts of 6), `.github/workflows/ci.yml`, the SQL migration list and `001_init.sql`,
`instances/afk.rs`, `instances/rejoin.rs` (first 140 lines and the backoff function), and the parts
of `instances/tracker.rs` that select anti-AFK clients and handle the idle kick.

Not read: most of the roughly 26,000 lines of Rust and the React frontend. Statements about those
parts below come from the README, the docs and file sizes only, and are marked as such.

## What the reference is

A Windows-only account manager and multi-client launcher for Roblox, built with Tauri 2.12,
Rust (edition 2024), React 19, TypeScript, Vite, Tailwind 4 and TanStack Query. It is shipped as
version 0.9.2 with an Inno Setup installer, CI and a release workflow.

## Implemented, per code and docs

| Area | Evidence |
|---|---|
| Encrypted vault: Windows DPAPI, or Argon2id + AES-256-GCM master password, recovery code, encrypted `.roalt` backups | `src-tauri/src/vault/`, `Cargo.toml` (`argon2`, `aes-gcm`, `zeroize`), M1 |
| SQLite storage, WAL, nine migrations, SQLCipher-encrypted database | `storage/migrations/001..009`, `rusqlite` with `bundled-sqlcipher-vendored-openssl`, `PRAGMA journal_mode = WAL` |
| Accounts, groups, favourites, settings, crash history, server history | tables `accounts`, `groups`, `account_groups`, `favourites`, `settings`, `crashes`, `server_history`, `tracked` |
| Launching by game link, place ID, job ID, private server, share link, profile | `launcher/target.rs`, `launcher/uri.rs`, M3 |
| Multi-client tracking, bulk launch, "same server" | `instances/`, `launcher/bulk.rs`, M4-M5 |
| Auto-rejoin / auto-restart with crash breaker and exponential backoff | `instances/rejoin.rs` |
| Anti-AFK by sending a real F13 key press | `instances/afk.rs` (the presses and the 15-minute cadence were seen in log files from a real Windows run) |
| CPU, memory and affinity limits, working-set trims, metrics | `instances/limits.rs`, `instances/metrics.rs` |
| Discord webhook alerts, Roblox and self-update with signature checks | `discord.rs`, `launcher/update.rs`, `self_update.rs` |
| 18 UI languages | `src-tauri/locales`, `scripts/check-i18n.mjs` |

## Proposed or stale

- `docs/DESIGN.md` still begins "Status: proposal, 2026-10-24. Nothing is built yet", although
  the code is at 0.9.2. The document and the code are out of step.
- The README says the installer is not code-signed yet, and the repository has a `winget.yml`
  workflow; whether winget publishing works was not checked.

## Tests

- About 280 `#[test]` functions in Rust, run on Windows in CI (`cargo test`, `cargo clippy -D warnings`).
- No frontend test runner appears in `package.json`.
- Windows-only code can only be tested on Windows, so the audit environment (Linux) could not run
  any of it.

## Weaknesses and opportunities (inferred, not measured)

- **Very large files.** `tracker.rs` is about 2,400 lines, `commands.rs` about 2,550 and `model.rs`
  about 1,280. They are hard to review and test in pieces.
- **Polling.** The tracker loop polls and metrics refresh every 2 seconds, so CPU use scales with
  the number of clients. The spec asks for a state machine and adaptive intervals instead.
- **No frontend tests.**
- **One crate for everything.** Fine for an app, but it makes the Windows-specific code harder to
  isolate and the portable logic harder to test on other systems.
- **Heavy build.** `bundled-sqlcipher-vendored-openssl` compiles OpenSSL from source.

## Decisions for Nexora

Reimplement independently, with a different structure:

- Typed IPC generated from Rust structs (ts-rs), as in the reference.
- Envelope-style protection for stored credentials (Phase 8; see `SECURITY.md`).
- SQLite with WAL, migrations and bounded history.
- Exponential backoff with a retry limit and a crash breaker, driven by an explicit state machine.
- Adaptive monitoring intervals.

Not reimplemented, because the project spec rules them out or leaves them undecided:

- **Simulated key presses to defeat Roblox's idle kick.** The reference's anti-AFK does this. The
  spec says AFK management must not simulate input to defeat inactivity enforcement, so RoALT
  Advanced will offer idle detection, notifications and reminders only.
- **Anything that reads or writes a Roblox client's memory or injects code.** Excluded outright.
- **The reference's multi-instance mechanism (holding Roblox's single-instance lock, or duplicating
  its handle).** The project owner has since decided that several clients are supported, with
  duplicate protection for the same account + game combination. How that is achieved is a Phase 4
  design question, to be settled and documented then; the reference's technique is not assumed.

## Licence and attribution

No code was copied from the reference. If any is reused later, keep the MIT notice
("Copyright (c) 2026 inspectsoftware") with that code and list the file here.
