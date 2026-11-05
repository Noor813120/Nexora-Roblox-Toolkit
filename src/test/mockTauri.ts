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

export const APP_INFO = { name: "RoALT Advanced", version: "0.1.0", platform: "windows", arch: "x86_64" };

export const SYSTEM_INFO = {
  logicalCores: 8,
  memory: { totalBytes: 16 * GIB, usedBytes: 10 * GIB, availableBytes: 6 * GIB },
};
