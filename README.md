# Nexora — Roblox Toolkit

**Launch. Manage. Automate. Monitor.**

Nexora is an independent Windows desktop companion for Roblox players and developers. Its goal is to bring Roblox client management, development utilities, monitoring, and supported automation workflows into one convenient application.

> **Development status:** Early development — version 0.1.0.

## Project Goals

- **Launcher:** Launch and manage Roblox clients.
- **Multi-instance management:** Support multiple Roblox client instances.
- **Automation:** Provide supported development and workflow automation.
- **Script management:** Organize Luau scripts for supported development environments.
- **Performance monitoring:** Display available system and application metrics.
- **Activity logs:** Record application events and operations.
- **Settings and profiles:** Store preferences and organize configurations.
Nexora is an independent project. It was developed after auditing
[inspectsoftware/RoALT](https://github.com/inspectsoftware/RoALT) (MIT)
and shares no code with that project.

See [Reference Audit](docs/REFERENCE_AUDIT.md) and [Roadmap](docs/ROADMAP.md).

## Current Implementation

The current development build includes:

- Tauri 2 desktop application foundation.
- React 19 and TypeScript frontend.
- Rust backend.
- SQLite local database.
- Persistent application settings.
- Profile and automation-profile database foundations.
- Dashboard with available system information.
- Activity logging, filtering, and pagination.
- Database backup and restore.
- Database integrity checks and migration safeguards.
- Windows desktop interface and native file dialogs.

The following are not yet implemented as complete features:

- Roblox launcher and client management.
- Multi-instance management interface.
- Luau script management and execution.
- Automation workflows.
- Account management.

## Technology Stack

| Technology | Purpose |
|---|---|
| Tauri 2 | Desktop application framework |
| Rust | Backend |
| React 19 | User interface |
| TypeScript | Frontend development |
| Vite | Build tooling |
| SQLite | Local database |
| Tailwind CSS | Interface styling |

## Platform and Requirements

- Windows 10 (1809+) or Windows 11, 64-bit.
- Node.js 22 or newer and npm.
- Rust stable with the MSVC toolchain.
- Microsoft Visual Studio C++ Build Tools.
- Windows SDK.
- WebView2 Runtime.

Windows development checks, desktop startup, database creation, backup, and restore have been tested on Windows. Other operating systems are not currently supported or verified.

## Safety and Scope

Nexora is intended as a legitimate desktop companion and development toolkit.

It will not provide:

- Roblox client injection.
- Exploit execution or unauthorized code injection.
- Anti-cheat or security bypasses.
- Automation designed to defeat Roblox inactivity restrictions.

Any anti-AFK functionality is limited to idle detection and notifications. Luau tooling is intended for Roblox Studio and supported development environments.

## Data and Privacy

Nexora uses a local SQLite database for application data.

**Current Windows database location:**

`%LOCALAPPDATA%\RoALT-Advanced\roalt.db`

The data directory will be migrated to the Nexora identity as part of the application rename. Do not manually rename or move the existing database.

Database backup and restore functionality is implemented and tested. Sensitive account credentials are not currently stored by the application.

## Development

Install dependencies:
