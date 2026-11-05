# Security Policy

## Supported Versions

Nexora is currently in early development. Security fixes are prioritized for the latest development version.

| Version | Security Support |
|---|---|
| 0.1.x | Current development line |

## Reporting a Vulnerability

Please do not disclose security vulnerabilities through public GitHub issues.

If you discover a potential vulnerability:

1. Open the repository's **Security** tab on GitHub.
2. Use **Report a vulnerability** to submit a private vulnerability report, if available.
3. Include the affected version, steps to reproduce, potential impact, and any relevant technical details.

Please allow the maintainer reasonable time to investigate and address the report before publicly disclosing it.

## Known Dependency Advisory

As of November 5, 2026, the repository has an open GitHub Dependabot alert:

- **Advisory:** GHSA-wrw7-89jp-8q8g
- **Package:** `glib`
- **Affected version:** `0.18.5`
- **First fixed version:** `0.20.0`
- **Severity:** Moderate
- **Dependency:** Tauri's GTK/WebKit Linux desktop dependency stack.

The vulnerable GLib version is present in the Linux dependency graph through GTK-related dependencies. It was not present in the verified Windows MSVC dependency graph.

A direct GLib upgrade is not currently considered safe because the GTK-related dependencies use an interconnected older version family.

The alert remains open. This is an acknowledged outstanding dependency issue, not a claim that the entire project is vulnerability-free.

The dependency chain should be revisited when compatible upstream Tauri and GTK-related updates become available. Any update must be followed by appropriate dependency audits and platform build tests.

## Security Scope

Nexora is a Windows desktop application built with Tauri, React, TypeScript, and Rust.

Security reviews should consider:

- Local database integrity and backup/restore behavior.
- Handling of user-provided input and local files.
- Tauri commands and application permissions.
- Third-party dependencies and their advisories.
- Installer and update distribution integrity.

Security checks and successful builds reduce risk but do not guarantee that the application is free of vulnerabilities.