import { invoke } from "@tauri-apps/api/core";
import type { DetectionSource } from "./bindings/DetectionSource";
import type { DetectionStatus } from "./bindings/DetectionStatus";
import type { Finding } from "./bindings/Finding";
import type { ProbeOutcome } from "./bindings/ProbeOutcome";
import type { RobloxDetection } from "./bindings/RobloxDetection";
import type { RobloxInstall } from "./bindings/RobloxInstall";

export type { DetectionSource, DetectionStatus, Finding, ProbeOutcome, RobloxDetection, RobloxInstall };

/**
 * The Roblox service. Detection only reads: it never launches anything, and it takes no path or
 * command from the page. The answer is for the screen; it is not saved anywhere.
 */
export const robloxApi = {
  detectRoblox: () => invoke<RobloxDetection>("detect_roblox"),
};
