# RoALT Advanced

A Windows desktop app for managing Roblox accounts and launching games, built with Tauri 2, Rust,
React 19 and TypeScript.

**Status: Phase 2 (UI foundation).** The app has a sidebar with eight sections, a Dashboard with
real build and system information, and empty states for everything not built yet. There are no
accounts, launcher, automation or scripts yet. See [docs/ROADMAP.md](docs/ROADMAP.md).

This is an independent project. It was designed after auditing
[inspectsoftware/RoALT](https://github.com/inspectsoftware/RoALT) (MIT) and shares no code with it;
see [docs/REFERENCE_AUDIT.md](docs/REFERENCE_AUDIT.md).

## What it will not do

It will not inject code into Roblox, read or write a client's memory, run exploit executors, or send
input to defeat Roblox's idle kick. Anti-AFK means idle detection and notifications only. The Luau
tools are for Roblox Studio development only. Details in [docs/SECURITY.md](docs/SECURITY.md).

## Requirements

- Windows 10 (1809+) or 11, x64. WebView2 runtime (built into Windows 11).
- Node.js 22 or newer and npm.
- Rust (stable, 1.85+) with the MSVC toolchain and Visual Studio C++ Build Tools.
- Git.

On Linux the backend and frontend can be built and tested (install the Tauri 2 system packages
first), but the app targets Windows and **has not been run on Windows yet**.

## Commands

```bash
npm install            # frontend dependencies
npm run typecheck      # TypeScript check
npm run test           # frontend tests (Vitest)
npm run build          # typecheck + production frontend build
npm run bindings       # regenerate TypeScript types from the Rust structs
npm run tauri dev      # run the app with hot reload
npm run tauri build    # build the Windows app
cargo test --manifest-path src-tauri/Cargo.toml                           # Rust tests
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

## Layout

```
src/            React frontend (app, components, features, hooks, lib, api, styles, test)
src-tauri/      Rust backend (commands, automation, launcher, scripts, monitoring, storage, security, platform)
docs/           ARCHITECTURE, ROADMAP, SECURITY, PERFORMANCE, REFERENCE_AUDIT
```

## Licence

MIT is proposed but not yet confirmed by the project owner, so no `LICENSE` file has been added.
Until one is, all rights are reserved.
