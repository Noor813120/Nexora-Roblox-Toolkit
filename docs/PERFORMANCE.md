# Performance

Status: end of Phase 2. Only the frontend shell has been measured, on Linux, and the numbers
below say exactly how. There are no Windows measurements, and nothing here is a claim about CPU or
RAM use of the packaged app. The rules and targets are what later phases are held to.

## Measured in Phase 2

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
- SQLite in WAL mode, prepared statements, indexed queries, bounded history, batched writes.
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
