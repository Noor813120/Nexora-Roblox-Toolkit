# Architecture

Status: end of Phase 3. The shell, navigation, design system, Dashboard and the SQLite database
foundation exist. Sections marked *planned* describe the intended design, not working code.

## Project decisions (set by the project owner before Phase 2)

1. Independent rebuild; no code shared with the reference project.
2. Windows 10/11 x64.
3. **Several Roblox clients are supported.**
4. Duplicate protection detects duplicate **account + game** combinations; it does not forbid
   several instances.
5. **Anti-AFK is idle detection and notifications only.** The app never simulates input to defeat
   Roblox's inactivity enforcement.
6. Licence: MIT proposed, **not yet confirmed** by the project owner. No `LICENSE` file exists yet.
7. Balanced RAM, CPU and reliability optimization.

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
| `commands/` | The IPC surface. Typed request/response, validation, `AppError`. | 1+ | `app_info`, `system_info`, 18 database commands (`commands/database.rs`) and three Roblox commands (`commands/roblox.rs`): `detect_roblox`, `get_install_selection`, `set_preferred_install` |
| `error.rs` | `AppError` serialized as `{ code, message }`; codes `invalid`, `not_found`, `conflict`, `unavailable`, `internal`. | 1, 3 | done, tested, returned by every database command |
| `platform/` | Thin OS wrappers; the only place for `cfg(windows)`. | 1, 2, 4 | `os`, `arch`, `logical_cores`, `memory_status`, and the read-only Roblox probes (`platform/roblox.rs`) |
| `storage/` | SQLite: connection, migrations, settings, profiles, automation profiles, activity log, backup and restore. | 3 | done on Linux; see [DATABASE.md](DATABASE.md) |
| `launcher/` | Process discovery, launching, sessions, queue. | 4 | Milestones 1 and 2. Milestone 1: read-only Roblox installation detection (`launcher/roblox.rs`). It decides what the evidence means and is testable on any OS; `platform/roblox.rs` reads folders, file metadata and one registry value. An install means a non-empty regular file at an expected path; contents are not checked, so it is not proof of genuine Roblox. Anything unreadable or not understood gives "inconclusive", never "not detected"; a version-named link in the Versions folder is never followed and makes the folder listing incomplete. It never runs the registry command, starts a process or uses the network, and the detection result itself is not saved or logged. Microsoft Store installs are reported as not checked. Milestone 2 adds the saved preferred installation (`launcher/preference.rs`; see [Preferred installation](#preferred-installation-phase-4-milestone-2)). Nothing in `launcher/` launches anything yet. |
| `automation/` | Auto-join, recovery state machine, retries, cancellation. | 5 | empty |
| `scripts/` | Script Hub: import, external execution, Luau editing. | 6 | empty |
| `monitoring/` | RAM/CPU sampling, adaptive intervals, Resource Guard. | 7 | empty |
| `security/` | Credential protection, permission manifests, audit log. | 6, 8 | empty |

### IPC commands

| Command | Returns | Notes |
|---|---|---|
| `app_info` | `AppInfo { name, version, platform, arch }` | Static facts about the build. |
| `system_info` | `SystemInfo { logicalCores: number \| null, memory: MemoryInfo \| null }` | A snapshot read once per call. `memory` (`totalBytes`, `usedBytes`, `availableBytes`) comes from `GlobalMemoryStatusEx` on Windows and is `null` everywhere else. `null` means "unavailable"; the UI must never show a made-up number. |
| `detect_roblox` | `RobloxDetection { status, installs, findings }` | Read-only. No input and no database state; the result is not saved or logged. |
| `get_install_selection` | `InstallSelection { detection, preference, availableSources }` | Read-only. One detection, the saved preference compared against it, and the sources that could be chosen right now. `preference` is `null` when preference storage is unavailable or the saved preference cannot be read; this is not the same as "automatic". |
| `set_preferred_install` | `InstallSelection` | The only command that changes the preference. Argument: a required `choice` (`automatic`, `protocolHandler` or `versionFolder`), never a path. Saves and logs it in one transaction. Answers `conflict` if that source cannot be chosen right now or the system is not Windows; `automatic` is always allowed. |

None of these takes a path, a program or a credential, and none exposes a handle. The TypeScript
types are generated from the Rust structs (`npm run bindings`) into `src/api/bindings/`.

The database commands (settings, profiles, automation profiles, activity logs, status, backup,
restore) are listed in [DATABASE.md](DATABASE.md#ipc-commands). None accepts SQL or a file path.

### Preferred installation (Phase 4, Milestone 2)

The user can choose which kind of Roblox installation Nexora should prefer: the one the Windows
`roblox-player` link handler points to (`protocolHandler`), the newest one in Roblox's version
folders (`versionFolder`), or `automatic` (detection's first install). Nothing here launches Roblox.

Three things are kept apart:

- **Detection** (`launcher/roblox.rs`) says what exists on this computer. Milestone 2 does not change it.
- **The stored preference** is a source kind, never a path, because Roblox replaces its version
  folders on every update. It is one setting, `launcher.preferred_install`, holding
  `{"v":1,"source":"..."}`; no row means automatic. No migration was needed
  ([DATABASE.md](DATABASE.md#the-preferred-installation-setting)).
- **Resolution** (`launcher/preference.rs`, pure functions, no disk access) compares the two afresh
  every time and reports one of six states: automatic, available, unavailable, inconclusive,
  unsupported, invalid. A stored preference is never proof that an installation exists.

Rules:

- A preference that cannot be resolved is reported as such. It never falls back to another
  installation; only "automatic" uses detection's first install.
- `versionFolder` means the newest `RobloxPlayerBeta.exe` inside a version folder, including the
  folder the link handler points to. The pick is deterministic: newest file date first, unknown dates
  last, then the folder name (ASCII-lowercased, then as written), then the path, then the source
  (link handler before version folders), so it never depends on the order detection listed things
  in. With several candidates the screen shows "1 of N".
- "Could not be checked" is never reported as "not there". A source with no installs is
  *unavailable* only after a clean "no" from that source's own checks, otherwise *inconclusive*, and
  one unreadable source does not hide the other.
- A stored value that cannot be used is left exactly as it is and reported: *invalid* (not something
  this version wrote), or *unsupported* (written by a newer version, naming a source that cannot be
  preferred, or saved on a system where it cannot be used). Only an explicit choice by the user
  replaces it.
- `availableSources` lists the sources whose choice would be accepted right now, which is the rule
  `set_preferred_install` applies, in a fixed order. The screen offers only those, plus "Automatic",
  which is always allowed and always offered.
- `set_preferred_install` detects once (not at all if the database is unavailable), before it takes
  the database lock, rejects a source that is not available, then changes the setting and writes the
  activity entry in one transaction (both or neither). Choosing what is already chosen, however it
  is spelled in storage, writes and logs nothing.
- Detection is a point-in-time observation. Anything that later acts on it (launching, in a future
  milestone) must check again immediately before acting.

In the Game Launcher card, the preference panel shows the state, the options that can be chosen and an
explicit "Save choice". After a save, the card re-reads the whole selection and shows what the backend
now says; it does not trust the save reply. If the preference cannot be read the whole control is
disabled and is not shown as "automatic". A saved source that is not available stays selected but
disabled; a stored value this version cannot read or use (invalid, from a newer version, or naming a
source that cannot be preferred) leaves no option selected, and "Automatic" can replace it.

## Frontend (`src/`)

```
app/            App root, navigation table, route hook, lazy page map
components/
  layout/       AppShell, Sidebar, PageLayout, PlannedPage, DatabaseBanner
  ui/           Card, StatCard, StatusPill, EmptyState, Button, ErrorBoundary, PageSkeleton
features/       one folder per sidebar section (dashboard, accounts, launcher, automation,
                scripts, performance, logs, settings), each exporting one page
hooks/          useBackend (reads), useAction (writes), useActivityLog (paged log)
lib/            format (bytes, percent, clock, date-time), errors (describeError)
api/            typed IPC wrappers (index.ts, database.ts) and generated bindings (the only place
                that calls invoke)
styles/         Tailwind entry and design tokens
test/           test setup and a Tauri mock for unit tests
```

### Navigation decisions

| Decision | Reason |
|---|---|
| Hash routing (`#/accounts`), no router library | Eight static pages need no router. Hash URLs need no server and work in the Tauri webview. One dependency avoided. |
| One table, `NAV_ITEMS`, drives the sidebar, the Dashboard's quick links and the tests | Order, labels, paths and icons cannot drift apart. A test pins the order from the spec. |
| Unknown or empty hash shows the Dashboard | A stale or hand-typed URL never gives a blank screen. |
| Every page is lazy-loaded (`pages.ts`) | A session downloads only the pages it opens. |
| Pages are wrapped in an `ErrorBoundary` keyed by route | One page crashing leaves the sidebar working, and opening another page clears the error. |
| On navigation: window title updates and focus moves to `<main>` | Keyboard and screen-reader users land at the top of the new content. A "Skip to content" button is the first tab stop. |
| `aria-current="page"` on the active link; icons are `aria-hidden` | Standard navigation semantics. |
| Not built: keyboard shortcuts for sections, a collapsible sidebar, a light theme | Not needed yet. |

### Component catalog

| Component | File | Purpose |
|---|---|---|
| `AppShell` | `components/layout/AppShell.tsx` | Window frame: skip link, sidebar, page area with error boundary and Suspense; title and focus handling. |
| `Sidebar` | `components/layout/Sidebar.tsx` | The eight sections. Memoized; re-renders only when the active route changes. |
| `PageLayout` | `components/layout/PageLayout.tsx` | One `h1`, optional description and actions, then content. |
| `PlannedPage` | `components/layout/PlannedPage.tsx` | A page for an unbuilt feature: description, empty state and a "Planned for Phase N" label. No fake controls. |
| `Card` | `components/ui/Card.tsx` | The one surface: flat, 1px border, optional title as `h2` that names the section. |
| `StatCard` | `components/ui/StatCard.tsx` | Dashboard tile: label, value, status badge, explanation. |
| `StatusPill` | `components/ui/StatusPill.tsx` | Short status text; colour only reinforces the words. |
| `EmptyState` | `components/ui/EmptyState.tsx` | Icon, title, description, optional phase label and real action. |
| `Button` | `components/ui/Button.tsx` | `primary` and `ghost` variants; `type="button"` by default. |
| `ErrorBoundary` | `components/ui/ErrorBoundary.tsx` | Fallback with "Try again"; logs to the console only. |
| `PageSkeleton` | `components/ui/PageSkeleton.tsx` | Loading placeholder; pulses only when motion is allowed. |
| `useBackend` | `hooks/useBackend.ts` | Loads one backend value on mount and on `reload()`; states `loading`, `offline`, `error`, `ready`; drops stale responses; never polls. |
| `useAction` | `hooks/useAction.ts` | Wraps a write (save, backup...): pending / done / error state, ignores a second call while one runs, no state update after unmount. |
| `useActivityLog` | `hooks/useActivityLog.ts` | Newest-first activity entries a page at a time; filter change restarts, `loadMore` appends without duplicates, stale replies dropped. |
| `DatabaseBanner` | `components/layout/DatabaseBanner.tsx` | Shown above every page only when the database is unavailable or a damaged one was replaced at startup. |
| `SeverityPill` | `features/logs/SeverityPill.tsx` | Severity as a word, with colour as reinforcement. |

### The honesty rule for data

Real backend data is shown as is. Anything the app cannot measure yet is shown as "—" with a label
("Not available yet", "Usage not measured yet", "Unavailable") and a sentence saying why and which
phase delivers it. There is no mock data in the app. The Roblox-client card says "This is not a
count of zero". The Memory card says it is the whole computer, not this app or Roblox.

### Design system

Tokens are defined once in `styles/index.css` (`@theme`): canvas, surface, raised, line, ink,
muted, one accent, and ok/warn/danger. Flat surfaces, 1px borders, no gradients, no shadows.
System font stack (Segoe UI Variable on Windows), so no font is downloaded. Icons are Lucide, imported
by name. Transitions are colour-only and a short fade on page change; both are disabled when the
operating system asks for reduced motion. One focus ring (2px accent) is used everywhere.

## Other decisions

| Decision | Reason |
|---|---|
| One Rust crate (binary), modules inside | Simplest to build and test. Split only if build times or isolation demand it. |
| ts-rs for shared types | Types come from the Rust structs, so the two sides cannot drift apart. |
| State machines instead of poll loops *(planned)* | Process exits and launches become events. |
| `cfg(windows)` only under `platform/` | Keeps the rest of the logic testable on any OS. |
| Plain SQLite via rusqlite, whole file not encrypted *(done, Phase 3)* | Avoids compiling OpenSSL for SQLCipher; the file holds ordinary configuration, and secrets will be protected per field (Phase 8). Details and the rusqlite-versus-SQLx comparison in [DATABASE.md](DATABASE.md). |
| Database work on blocking threads, one connection | Disk access never runs on the async runtime or the UI thread; one connection behind a mutex is the simplest safe model for one user and tiny queries. |
| The frontend never supplies a file path | Backup and restore open the native file dialog from Rust, so React cannot point the backend at arbitrary files. |
| Preferred installation stored as a source kind, never a path *(done, Phase 4)* | Roblox replaces its version folders on every update, so a stored path would go stale. |
| Detection runs once (not at all without a database), before the database lock *(done, Phase 4)* | File and registry reads never hold up the database, and a choice is checked against what is on the computer right now. |
| Vitest + Testing Library + jsdom for frontend tests | Reuses the Vite config; four dev-only dependencies, none in the shipped app. |
| Edition 2024, `panic = "abort"`, LTO, `opt-level = "s"` in release | Smaller, faster binary. |

## The database layer (Phase 3)

```
React (typed api/database.ts)
   |  invoke("list_profiles", ...)            no SQL, no paths
commands/database.rs   validate -> spawn_blocking -> convert errors; owns DbState (Option<Database>)
   |
storage/               settings . profiles . automation_profiles . activity_logs . backup
   |                   validation . errors . migrations . clock
database.rs            open, configure, identity + integrity check, recovery (quarantine)
   |
SQLite file (WAL)      %LOCALAPPDATA%\RoALT-Advanced\roalt.db  (Windows, intended)
```

If the database cannot be opened at startup the window still opens: `DbState` holds no database,
every database command answers `unavailable`, and a banner and the Settings page explain why. See
[DATABASE.md](DATABASE.md) for the schema, migrations, recovery, backup and security decisions.

## Not done / unknown

## Known limitations and unverified areas

- **Windows verification:** The Windows GitHub Actions build and test workflow passed on commit
  `8607635`. The later Phase 4 Milestone 2 changes have also passed the Rust and frontend test
  suites on Windows locally, but those changes have not yet been verified by a subsequent CI run.
- The Windows-only memory code (`GlobalMemoryStatusEx`) was compiled in the Windows build and its
  Windows tests were executed as part of the Windows test run.
- The UI has been viewed in headless Chromium (mocked backend) and in the real Tauri app under a
  virtual display on Linux (WebKitGTK). It has not been manually checked in WebView2.
- The placeholder icon has not been replaced.
- The database has been exercised by automated tests on Windows and in the real app on Linux.
  However, the real app's Windows data-folder location (`%LOCALAPPDATA%`), file renaming over an
  existing file, and the Windows save dialog's overwrite prompt remain unverified. Real backup and
  restore through native dialogs have been exercised on Linux only.
- **Preferred installation (Phase 4, Milestone 2), partially verified:**
  - Its Rust and frontend tests have passed on Windows locally. The Milestone 2 changes have not
    yet been run through GitHub Actions or tested on Linux.
  - Fourteen malformed IPC requests were rejected through Tauri's mock IPC boundary without
    changing the database. All three valid choices were also tested through the real command,
    including their persisted results.
  - The two live regions in the Game Launcher card (detection headline and preference status)
    have been checked in tests for structure and persistence, but not with a screen reader.
  - Restoring the preference on another computer was simulated with a second database and
    made-up machines, not tested with a second physical computer.
  - The manual run of Milestone 2 in the real app is pending.
