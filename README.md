# RoALT Advanced

A Windows desktop app for managing Roblox accounts and launching games, built with Tauri 2, Rust,
React 19 and TypeScript.

**Status: Phase 1 (foundation).** The project builds a window that calls one typed backend command.
There are no accounts, launcher, automation or scripts yet. See [docs/ROADMAP.md](docs/ROADMAP.md).

This is an independent project. It was designed after auditing
[inspectsoftware/RoALT](https://github.com/inspectsoftware/RoALT) (MIT) and shares no code with it;
see [docs/REFERENCE_AUDIT.md](docs/REFERENCE_AUDIT.md).

## What it will not do

It will not inject code into Roblox, read or write a client's memory, run exploit executors, or send
input to defeat Roblox's idle kick. The Luau tools are for Roblox Studio development only. Details
in [docs/SECURITY.md](docs/SECURITY.md).

## Requirements

- Windows 10 (1809+) or 11, x64. WebView2 runtime (built into Windows 11).
- Node.js 22 or newer and npm.
- Rust (stable, 1.85+) with the MSVC toolchain and Visual Studio C++ Build Tools.
- Git.

On Linux the backend can be compiled and unit-tested (install the Tauri 2 system packages first),
but the app is built for Windows.

## Commands

```bash
npm install            # frontend dependencies
npm run typecheck      # TypeScript check
npm run build          # typecheck + production frontend build
npm run bindings       # regenerate TypeScript types from the Rust structs
npm run tauri dev      # run the app with hot reload
npm run tauri build    # build the Windows app
cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests
```

## Layout

```
src/            React frontend (app/, api/, styles/)
src-tauri/      Rust backend (commands, automation, launcher, scripts, monitoring, storage, security, platform)
docs/           ARCHITECTURE, ROADMAP, SECURITY, PERFORMANCE, REFERENCE_AUDIT
```

## Licence

No licence has been chosen yet. Until one is added, all rights are reserved.
