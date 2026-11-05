# Roadmap

A phase is marked done only when its code builds and its tests pass. Untested features are never
marked complete.

| Phase | Scope | Status |
|---|---|---|
| 1. Foundation | Repo audit, Tauri 2 + React + TypeScript project, Rust module layout, docs, Git, build | Done on Linux; Windows build unverified |
| 2. User interface | Sidebar, navigation, dashboard shell, reusable components, theme system | Not started |
| 3. Database | SQLite, schema, migrations, settings, profiles, activity logs | Not started |
| 4. Launcher | Process discovery, Roblox launching, session tracking, launch queue | Not started |
| 5. Automation | Auto-join, recovery state machine, retry management, cancellation, session monitoring | Not started |
| 6. Script Hub | Import, external script execution, Luau editor, permissions, execution logs | Not started |
| 7. Performance | RAM and CPU monitoring, adaptive polling, Resource Guard, benchmarks | Not started |
| 8. Security | Script security, credential protection, IPC validation, security tests | Not started |
| 9. Release | Windows installer, build automation, GitHub Actions, release verification | Not started |

## Scope notes that change the original spec's wording

- **AFK Management** means idle detection, notifications and reminders. It does not send key
  presses or other input to keep a client from being idle-kicked.
- **Script Hub, Luau** means editing and validating scripts for Roblox Studio. It does not run
  code inside a Roblox client.
- **Multi-instance** is an open decision for Phase 4, because the launcher spec also says to
  prevent accidental duplicate launches.

## Open items after Phase 1

- Run `npm run tauri dev` and `cargo test` on Windows 10/11 and record the result.
- Decide the project licence (none has been added yet).
- Replace the placeholder icon.
