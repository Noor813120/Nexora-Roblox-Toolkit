# Performance

Status: Phase 1. Nothing has been measured yet, so this document makes no performance claims. It
records the rules and targets that later phases are held to.

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
