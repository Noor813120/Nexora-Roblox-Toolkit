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

/** Handlers for every read the app makes on load, so a test only overrides what it cares about. */
export function backend(overrides: Record<string, Handler> = {}): Record<string, Handler> {
  return {
    app_info: () => APP_INFO,
    system_info: () => SYSTEM_INFO,
    database_status: () => DB_STATUS,
    list_settings: () => SETTINGS,
    list_activity_logs: () => ({ items: [], nextBeforeId: null }),
    ...overrides,
  };
}
