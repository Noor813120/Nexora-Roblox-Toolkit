# Roadmap

A phase is marked done only when its code builds and its tests pass. Untested features are never
marked complete.

| Phase | Scope | Status |
|---|---|---|
| 1. Foundation | Repo audit, Tauri 2 + React + TypeScript project, Rust module layout, docs, Git, build | Done on Linux; Windows build unverified |
| 2. User interface | Sidebar, navigation, dashboard, reusable components, design system, frontend tests | Done on Linux; Windows build unverified |
| 3. Database | SQLite, schema, migrations, settings, profiles, activity logs, backup and restore | Done on Linux; Windows unverified |
| 4. Launcher | Process discovery, Roblox launching, session tracking, launch queue | In progress: Milestones 1 and 2 implemented; automated checks pass; Windows manual checks completed. Launching, process discovery, session tracking, and launch queue remain unimplemented |
| 5. Automation | Auto-join, recovery state machine, retry management, cancellation, session monitoring | Not started |
| 6. Script Hub | Import, external script execution, Luau editor, permissions, execution logs | Not started |
| 7. Performance | RAM and CPU monitoring, adaptive polling, Resource Guard, benchmarks | Not started |
| 8. Security | Script security, credential protection, IPC validation, security tests | Not started |
| 9. Release | Windows installer, build automation, GitHub Actions, release verification | Not started |

## Phase 2: what exists

- Eight-section sidebar, hash navigation, lazy-loaded pages, error boundary per page.
- Dashboard: application status, Roblox-client tile, CPU, memory, automation status, recent
  activity, quick navigation. Real data: build info, logical core count, and (Windows only) a
  physical-memory snapshot. Everything else is labelled as not available yet.
- Seven other pages with honest empty states and a "Planned for Phase N" label; Settings shows the
  real build information.
- Rust: `system_info` command; `platform::logical_cores` and `platform::memory_status`.
- Tests: 16 Rust, 47 frontend.

## Phase 3: what exists

- SQLite (rusqlite, bundled) in the OS local-data folder: WAL, foreign keys enforced, versioned
  migrations with checksums and a safety copy before upgrades, startup integrity check, damaged files
  moved to a quarantine folder instead of deleted.
- Tables: settings, profiles, automation profiles (configuration only), activity logs (bounded to
  10,000 rows plus a retention setting), migration history.
- 20 typed IPC commands; none accepts SQL or a file path.
- Backup (consistent single file, validated, no silent overwrite) and restore (validated, safety
  copy, atomic load, automatic rollback), both through native dialogs opened from Rust.
- UI: real activity on the Dashboard, a real Activity Logs page (filter, paging, clean-up), real
  persisted preferences and a Database card in Settings, a banner for database problems.
- Tests: 125 Rust (86 in storage), 115 frontend. Not built on purpose: any profile UI, launcher,
  automation, credential storage.
- Full details: [DATABASE.md](DATABASE.md).

## Phase 4: what exists so far

- Milestone 1: read-only Roblox installation detection with a Game Launcher card (installs found, what was checked, "Check again"). Microsoft Store installs are reported as not checked. The detection result is not saved or logged.
- Milestone 2: a saved preferred installation (the link handler's, the newest in the version folders, or automatic), compared with fresh detection and shown with its state. Changes are saved explicitly and logged; there is no schema change. Details: [ARCHITECTURE.md](ARCHITECTURE.md#preferred-installation-phase-4-milestone-2).
- Tests: 251 Rust passed, 1 ignored; 189 frontend passed. GitHub Actions CI passed for commit `800193f`.
- Milestone 2 manual Windows checks (November 8, 2026): application launch, Roblox detection, installation source/path reporting, preferred source selection, persistence after restart, and activity logging passed.
- Screen-reader announcements: not tested.
- Not built on purpose: launching Roblox, process discovery, sessions and the launch queue.
## Project decisions (from the project owner)

- Several Roblox clients are supported. Duplicate protection flags duplicate **account + game**
  combinations (Phase 4); it does not prohibit multiple instances.
- AFK management is idle detection and notifications. It does not send key presses or other input
  to keep a client from being idle-kicked.
- Script Hub: Luau means editing and validating scripts for Roblox Studio. It does not run code
  inside a Roblox client.
- Licence: MIT proposed, pending the project owner's confirmation.
- Optimization is balanced across RAM, CPU and reliability.

## Open items

- Run `npm install`, `npm run tauri dev`, `npm run test` and `cargo test` on Windows 10/11 and
  record the results. Until then no Windows claim is made.
- Confirm the licence, then add a `LICENSE` file.
- Replace the placeholder icon.
- Decided in Phase 3 (and to be confirmed by the owner if they disagree): the data folder is
  `%LOCALAPPDATA%\RoALT-Advanced`, and the database file is **not** encrypted as a whole; secrets will
  be protected per field in Phase 8.
- Phase 3 Windows checks: location, backup and restore dialogs, and WAL behaviour next to antivirus.
- Screen-reader testing of the Game Launcher card live regions remains outstanding.
- The Settings page asynchronous-save test was stabilized with `waitFor` and CI test configuration changes. The underlying cause of the earlier intermittent failure has not been established.
