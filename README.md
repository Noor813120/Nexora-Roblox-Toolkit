# Nexora — Roblox Toolkit

[![MIT License](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/Noor813120/Nexora-Roblox-Toolkit/releases/tag/v0.1.0)
[![Platform](https://img.shields.io/badge/platform-Windows-blue.svg)](https://github.com/Noor813120/Nexora-Roblox-Toolkit)

**Launch. Manage. Automate. Monitor.**

Nexora is an independent Windows desktop companion for Roblox players and developers. Its goal is to bring Roblox client management, development utilities, monitoring, and supported automation workflows into one convenient application.

> **Development status:** Early development — version 0.1.0. Many planned features are not yet implemented.

## Download

**Latest release:** [Nexora v0.1.0 — Early Development](https://github.com/Noor813120/Nexora-Roblox-Toolkit/releases/tag/v0.1.0)

Available Windows installers:

| Installer | Description |
|---|---|
| `Nexora_0.1.0_x64-setup.exe` | NSIS Windows installer |
| `Nexora_0.1.0_x64_en-US.msi` | Windows MSI installer |

Choose one installer rather than installing both.

Download the installer from the official GitHub Releases page. Back up important data before installing development builds.

## SHA-256 Checksums

Use SHA-256 to verify that your downloaded installer matches the published release.

| File | SHA-256 |
|---|---|
| EXE | `0AA0067E6FD79F63E44451506D82F9D852110A570E9FA7C25DC4DA1E990BD2B9` |
| MSI | `8F45793B29EEB548E90B7530793E7B787C25036A512B123C1348F0FEB1074246` |

### Verify on Windows

Open PowerShell in the folder containing your downloaded installer.

For the EXE:

```powershell
Get-FileHash .\Nexora_0.1.0_x64-setup.exe -Algorithm SHA256
```

For the MSI:

```powershell
Get-FileHash .\Nexora_0.1.0_x64_en-US.msi -Algorithm SHA256
```

Compare the resulting hash with the corresponding value above. The values must match exactly.

A matching checksum verifies file integrity against the published hash; it does not independently establish that the software is safe or digitally signed.

## Project Goals

- **Launcher:** Launch and manage Roblox clients.
- **Multi-instance management:** Support multiple Roblox client instances.
- **Automation:** Provide supported development and workflow automation.
- **Script management:** Organize Luau scripts for supported development environments.
- **Performance monitoring:** Display available system and application metrics.
- **Activity logs:** Record application events and operations.
- **Settings and profiles:** Store preferences and organize configurations.

Nexora is an independent project developed after auditing [inspectsoftware/RoALT](https://github.com/inspectsoftware/RoALT) (MIT). The original repository served as a reference; Nexora maintains a separate codebase.

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

### Not Yet Implemented

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

### For Users

- Windows 10 (version 1809 or newer) or Windows 11.
- 64-bit Windows.
- Microsoft Edge WebView2 Runtime.

### For Developers

- Node.js 22 or newer.
- npm.
- Rust stable with the MSVC toolchain.
- Microsoft Visual Studio C++ Build Tools.
- Windows SDK.

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

```text
%LOCALAPPDATA%\RoALT-Advanced\roalt.db
```

This legacy data location and the existing application identifier are intentionally preserved for compatibility with existing installations. Do not manually rename or move the database.

Database backup and restore functionality is implemented and tested. Sensitive account credentials are not currently stored by the application.

## Development

Clone the repository:

```powershell
git clone https://github.com/Noor813120/Nexora-Roblox-Toolkit.git
cd Nexora-Roblox-Toolkit
```

Install dependencies:

```powershell
npm ci
```

Run the desktop application in development mode:

```powershell
npm run tauri dev
```

Run frontend checks and tests:

```powershell
npm run check
```

Build the Windows installers:

```powershell
npx tauri build
```

Generated installer files are located under:

```text
src-tauri/target/release/bundle/
```

## License

Nexora is licensed under the [MIT License](LICENSE).

Copyright (c) 2026 Noor813120
