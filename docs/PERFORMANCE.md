# Performance

Status: end of Phase 3. Phase 2 measured the frontend shell; Phase 3 adds measurements of the
database layer. Everything was measured on Linux and the numbers say exactly how. There are no
Windows measurements, and nothing here is a claim about CPU or RAM use of the packaged app. The rules
and targets are what later phases are held to.

## Measured in Phase 3 (database layer)

Setup: **release build**, 2 vCPU Intel Xeon @ 2.1 GHz virtual machine, `ext4` on a virtual disk,
SQLite with `journal_mode = WAL` and `synchronous = FULL` (every commit waits for the disk). Timing
code: `src-tauri/src/storage/perf_tests.rs`, run with
`cargo test --release storage::perf_tests -- --ignored --nocapture --test-threads=1`. Averages over
the repetitions shown. **These are database-layer timings only**: they exclude IPC serialisation,
the webview and rendering. Disk-sync cost on a Windows PC (NTFS, a different SSD, antivirus) will
differ, and the commit numbers are the ones most likely to.

| Measurement | Result |
|---|---|
| Open and migrate a new database file | 4.5 ms (once, at startup) |
| Reopen a database holding 10,000 log rows, including `quick_check` | 15.3 ms |
| `create_activity_log`, one commit each (500) | 218 µs |
| `create_activity_log` with the table at its 10,000-row cap, including trimming (200) | 293 µs |
| Rows after those extra inserts | 10,000 (the cap holds) |
| `list_activity_logs`, newest 50 of 10,000 (500) | 43 µs |
| Same, filtered to errors (500) | 57 µs |
| Same, deep page from the oldest end (500) | 14 µs |
| Main database file at the cap (10,000 rows with messages of about 80 characters) | 1,652 KB |
| Backup of that database (`VACUUM INTO` plus validation), 1,592 KB output | 26.6 ms |
| Restore of that backup (copy, validate, safety copy, load, re-check) | 46.9 ms |

What this says: at the history cap, every storage call is well under a millisecond and a
backup/restore of the largest database the app allows takes under a tenth of a second, so none of
them needs to be treated as a long-running task. What it does not say: how the UI feels (not
measured), memory use of the process (not measured), or results on Windows (not measured).

Query plans are checked by a test: severity and event-type filters use their indexes, deletion by
age uses the timestamp index, and newest-first listing needs no sort.

### Frontend changes in Phase 3

Production build output (Vite), compared with the Phase 2 figures below:

| Measurement | Phase 2 | Phase 3 |
|---|---|---|
| Main JS | 221.64 kB raw, 70.13 kB gzip | 225.55 kB raw, 71.32 kB gzip |
| CSS | 14.49 kB raw, 3.86 kB gzip | 19.54 kB raw, 4.62 kB gzip |
| Dashboard page chunk | not recorded | 6.17 kB raw, 2.36 kB gzip |
| Settings page chunk (loaded on first visit) | 0.3-1.9 kB gzip class | 9.20 kB raw, 2.96 kB gzip |
| Activity Logs page chunk (loaded on first visit) | same | 7.41 kB raw, 2.63 kB gzip |

Backend calls to show the Dashboard: **4** (`app_info`, `system_info`, `list_settings`,
`list_activity_logs`; Phase 2 had 2). The last two run one after the other because the number of
entries to ask for is a saved setting. A unit test asserts both the four calls and that **no further
call happens during ten simulated idle minutes** (nothing polls); this is a test with a fake timer,
not a measurement in a browser. The Logs page asks for 50 entries per page and loads older pages only
on request. Not re-measured in Phase 3: JS heap, DOM node count, paint timings.

## Measured in Phase 2 (frontend shell, unchanged)

Setup: production build served by `vite preview`, headless Chromium 1200x760 on a 2-core Linux
container, backend replaced by a mock that answers instantly. This is **not** WebView2 and **not**
the packaged app; treat the numbers as a comparison point for later phases, not as results for a
user's PC.

| Measurement | Result |
|---|---|
| Main JS bundle | 221.64 kB raw, 70.13 kB gzip (Phase 1: 221.12 kB, 69.22 kB; React dominates) |
| CSS | 14.49 kB raw, 3.86 kB gzip (Phase 1: 6.51 kB, 2.24 kB) |
| Files loaded to show the Dashboard | 7 (HTML, CSS, 5 JS chunks) |
| JS loaded to show the Dashboard | 240.74 kB raw, about 78 kB gzip |
| Each other page | 1 extra small chunk (0.3-1.9 kB gzip), loaded on first visit; Accounts loads 2 |
| Shared icon chunk | 12.11 kB raw, 4.76 kB gzip (the Lucide icons used by the pages) |
| Backend calls to show the Dashboard | 2 (`app_info`, `system_info`) |
| Backend calls during 10 s of idle | 0 more (still 2): nothing polls |
| JS heap after load (Dashboard) | 3.4 MB |
| DOM nodes (Dashboard) | 306 |
| DOMContentLoaded / first contentful paint | 112 ms / 240 ms (headless, local, mocked backend) |
| Console errors across all eight pages | 0 |

Things not measured: RAM and CPU of the real Tauri process, startup time in WebView2, dev-server
start time (the Lucide barrel import makes Vite transform about 2,000 modules in a production build,
which took 0.4 s here), and React re-render counts. The Sidebar is memoized and routes change only
through the URL hash, but no render profile has been taken.

Also seen: running the real Tauri app (Linux, WebKitGTK, virtual display) loaded and showed live
data from the real backend. No memory or CPU figures were taken from that run.

## Measurement rules

- Measure **working set**, **private bytes** (commit) and **CPU time** separately and report all
  three. Working-set trimming lowers the working set for a while but does not reduce commit size or
  permanently reduce memory use, and must not be described as if it did.
- Benchmarks state the machine, the number of clients and the duration. A number without those is
  not a result.

## Design rules

- Prefer events to polling. A process exit or launch result is delivered as an event.
- Poll only for numbers that have no event (CPU, memory), at adaptive intervals.
- No background thread unless it has a job; no WebView beyond the main window.
- Reuse one HTTP client. Bound every cache and every in-memory log.
- SQLite in WAL mode, prepared statements, indexed queries, bounded history, batched writes
  (all in place since Phase 3; see the measurements above). Database calls run on blocking threads,
  never on the async runtime or the UI thread. A write that would not change anything is skipped.
- Lazy-load heavy frontend modules (the script editor in particular). Avoid re-rendering lists on
  every metrics tick.

## Starting monitoring intervals (all configurable, Phase 7)

| State | Interval |
|---|---|
| Active client | 2 s |
| Background client | 5-10 s |
| Minimized client | 15-30 s |
| Application window inactive | Suspend UI updates that nobody can see |

These are starting points to be tuned by measurement, not results.

## Resource Guard (Phase 7)

Configurable limits: concurrent launches, queued launches, RAM warning threshold, CPU monitoring,
and optional refusal to launch when available memory is too low.
