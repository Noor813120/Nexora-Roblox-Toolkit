import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./bindings/AppInfo";
import type { MemoryInfo } from "./bindings/MemoryInfo";
import type { SystemInfo } from "./bindings/SystemInfo";

export type { AppInfo, MemoryInfo, SystemInfo };

/**
 * One typed wrapper per backend command. The request and response types are generated from the
 * Rust structs (`npm run bindings`), so the two sides cannot drift apart silently.
 */
export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  systemInfo: () => invoke<SystemInfo>("system_info"),
};

/** True when running inside the Tauri window (false in a plain browser tab, e.g. `vite` alone). */
export const inTauri = (): boolean => "__TAURI_INTERNALS__" in window;
