import { vi } from "vitest";

type Handler = (args?: unknown) => unknown;

/**
 * Pretends to be the Tauri runtime for one test: `invoke(cmd)` calls `handlers[cmd]`, resolving to
 * its return value or rejecting if it throws. Returns the spy so tests can count calls.
 *
 * This replaces the Rust backend in unit tests only; it is not a substitute for running the app.
 */
export function mockTauri(handlers: Record<string, Handler>) {
  const invoke = vi.fn(async (cmd: string, args?: unknown) => {
    const handler = handlers[cmd];
    if (!handler) throw { code: "not_found", message: `no handler for ${cmd}` };
    return handler(args);
  });
  (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = { invoke, transformCallback: () => 0 };
  return invoke;
}

export const GIB = 1024 ** 3;

export const APP_INFO = { name: "Nexora", version: "0.1.0", platform: "windows", arch: "x86_64" };

export const SYSTEM_INFO = {
  logicalCores: 8,
  memory: { totalBytes: 16 * GIB, usedBytes: 10 * GIB, availableBytes: 6 * GIB },
};

// ---- database fixtures ----

export const DB_STATUS = {
  available: true,
  location: "C:\\Users\\test\\AppData\\Local\\RoALT-Advanced\\roalt.db",
  schemaVersion: 1,
  journalMode: "wal",
  sizeBytes: 53_248,
  recoveredFile: null,
  error: null,
};

export const SETTINGS = [
  { key: "activity.retention_days", value: "30", defaultValue: "30", isDefault: true, min: 1, max: 365, updatedAt: null },
  { key: "ui.dashboard_activity_count", value: "8", defaultValue: "8", isDefault: true, min: 3, max: 25, updatedAt: null },
];

type Severity = "debug" | "info" | "warning" | "error";

export const makeLog = (
  id: number,
  overrides: Partial<{ eventType: string; severity: Severity; message: string; metadataJson: string | null; createdAt: number }> = {},
) => ({
  id,
  eventType: "app.started",
  severity: "info" as Severity,
  message: `Entry ${id}`,
  metadataJson: null,
  createdAt: 1_762_000_000_000 + id * 1000,
  ...overrides,
});

// ---- Roblox detection fixtures ----

const STORE_FINDING = {
  source: "microsoftStore",
  outcome: "unsupported",
  detail: "Microsoft Store installs are not checked by this version, so one could exist even if none was found here.",
};

export const ROBLOX_NOT_DETECTED = {
  status: "notDetected",
  installs: [],
  findings: [
    { source: "protocolHandler", outcome: "notFound", detail: "Windows has no 'roblox-player' link handler registered." },
    { source: "versionFolder", outcome: "notFound", detail: "There is no Roblox Versions folder at C:\\Users\\test\\AppData\\Local\\Roblox\\Versions." },
    STORE_FINDING,
  ],
};

export const ROBLOX_DETECTED = {
  status: "detected",
  installs: [
    {
      source: "protocolHandler",
      version: "version-0123456789abcdef",
      executablePath: "C:\\Users\\test\\AppData\\Local\\Roblox\\Versions\\version-0123456789abcdef\\RobloxPlayerLauncher.exe",
      modifiedAt: 1_762_000_000_000,
    },
    {
      source: "versionFolder",
      version: "version-0123456789abcdef",
      executablePath: "C:\\Users\\test\\AppData\\Local\\Roblox\\Versions\\version-0123456789abcdef\\RobloxPlayerBeta.exe",
      modifiedAt: null,
    },
  ],
  findings: [
    {
      source: "protocolHandler",
      outcome: "found",
      detail: "The 'roblox-player' link handler points to an existing, non-empty program file. Contents were not checked.",
    },
    {
      source: "versionFolder",
      outcome: "found",
      detail:
        "Found 1 version folder(s) containing a non-empty RobloxPlayerBeta.exe; the most recently updated is listed first. Contents were not checked.",
    },
    STORE_FINDING,
  ],
};

export const ROBLOX_INCONCLUSIVE = {
  status: "inconclusive",
  installs: [],
  findings: [
    { source: "protocolHandler", outcome: "notFound", detail: "Windows has no 'roblox-player' link handler registered." },
    { source: "versionFolder", outcome: "inaccessible", detail: "The folder C:\\Users\\test\\AppData\\Local\\Roblox\\Versions could not be read." },
    STORE_FINDING,
  ],
};

/** A link handler that could not be understood, plus version-folder problems beyond the cap. */
export const ROBLOX_INCONCLUSIVE_MALFORMED = {
  status: "inconclusive",
  installs: [],
  findings: [
    {
      source: "protocolHandler",
      outcome: "malformed",
      detail: "The 'roblox-player' link handler has an unexpected type or size, so it could not be used.",
    },
    { source: "versionFolder", outcome: "missingExecutable", detail: "version-0000000000000000 has no RobloxPlayerBeta.exe." },
    {
      source: "versionFolder",
      outcome: "several",
      detail:
        "5 more version folder(s) had problems: 2 without RobloxPlayerBeta.exe, 3 with an empty or non-file RobloxPlayerBeta.exe.",
    },
    STORE_FINDING,
  ],
};

/** A version-named link was skipped (never followed), so the Versions listing is incomplete. */
export const ROBLOX_INCONCLUSIVE_INCOMPLETE = {
  status: "inconclusive",
  installs: [],
  findings: [
    { source: "protocolHandler", outcome: "notFound", detail: "Windows has no 'roblox-player' link handler registered." },
    {
      source: "versionFolder",
      outcome: "incomplete",
      detail: "The Roblox Versions folder could not be fully listed, so a version folder may have been missed.",
    },
    STORE_FINDING,
  ],
};

export const ROBLOX_UNSUPPORTED = {
  status: "unsupported",
  installs: [],
  findings: [{ source: "platform", outcome: "unsupported", detail: "Roblox detection is only available on Windows." }],
};

/** Handlers for every read the app makes on load, so a test only overrides what it cares about. */
export function backend(overrides: Record<string, Handler> = {}): Record<string, Handler> {
  return {
    app_info: () => APP_INFO,
    system_info: () => SYSTEM_INFO,
    database_status: () => DB_STATUS,
    list_settings: () => SETTINGS,
    list_activity_logs: () => ({ items: [], nextBeforeId: null }),
    detect_roblox: () => ROBLOX_NOT_DETECTED,
    ...overrides,
  };
}
