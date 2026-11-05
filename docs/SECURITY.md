# Security

Status: Phase 1. This document is the policy the code must meet. Almost none of it is implemented
yet; each item says which phase builds it.

## Principles

1. Never store a password or token in plaintext. (Phase 8)
2. Never bypass authentication or account security. Sign-in happens on Roblox's own page.
3. Never inject code into, or read or write the memory of, a Roblox client. No exploit executors,
   no anti-cheat bypasses, no DLL injection. This is out of scope for the whole project.
4. Never run an imported script automatically, and never run an untrusted script with unrestricted
   access to the machine. (Phase 6)
5. Never ask for or keep elevated privileges unless a single, user-confirmed action needs them.
6. The frontend is untrusted input. Every IPC command validates its arguments. (every phase)

## Current state (Phase 1)

| Control | State |
|---|---|
| Content Security Policy | Set in `tauri.conf.json`: same-origin scripts, `connect-src` limited to Tauri IPC, `frame-ancestors 'none'` |
| Capabilities | `main` window has `core:default` only |
| Shell, filesystem and HTTP plugins | Not installed |
| Stored secrets | None exist yet |
| Secrets in Git | `.gitignore` excludes `.env*`, key and certificate files, databases and `.roalt` files |
| IPC commands | One (`app_info`), read-only, no input |

## Planned controls

- **Script permission manifests.** Each script declares what it needs (network hosts, folders,
  running time). Nothing outside the manifest is allowed. (Phase 6)
- **Trusted and untrusted labels.** Imported scripts start untrusted. Running one needs an explicit
  confirmation that shows its manifest. (Phase 6)
- **Restricted execution.** Child process with a timeout, cancellation, output limits, a working
  directory of its own and a scrubbed environment (no credentials). (Phase 6)
- **Audit log.** Every script run, permission grant and credential access is recorded. (Phase 6, 8)
- **Credential protection.** Windows DPAPI-protected keys with per-field encryption, optional master
  password, no credentials in logs or IPC responses. (Phase 8)
- **IPC validation tests.** Each command has tests for malformed and hostile input. (Phase 8)

## Reporting a problem

Open a private security advisory on the project's repository once it has one.
