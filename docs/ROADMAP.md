# Roadmap

A phase is marked done only when its code builds and its tests pass. Untested features are never
marked complete.

| Phase | Scope | Status |
|---|---|---|
| 1. Foundation | Repo audit, Tauri 2 + React + TypeScript project, Rust module layout, docs, Git, build | Done on Linux; Windows build unverified |
| 2. User interface | Sidebar, navigation, dashboard, reusable components, design system, frontend tests | Done on Linux; Windows build unverified |
| 3. Database | SQLite, schema, migrations, settings, profiles, activity logs | Not started |
| 4. Launcher | Process discovery, Roblox launching, session tracking, launch queue | Not started |
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
- Phase 3 will need to decide where the app's data folder lives and whether the database file is
  encrypted as a whole or only its secrets.
