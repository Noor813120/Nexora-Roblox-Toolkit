import { invoke } from "@tauri-apps/api/core";
import type { DetectionSource } from "./bindings/DetectionSource";
import type { DetectionStatus } from "./bindings/DetectionStatus";
import type { Finding } from "./bindings/Finding";
import type { InstallChoice } from "./bindings/InstallChoice";
import type { InstallSelection } from "./bindings/InstallSelection";
import type { PreferenceReason } from "./bindings/PreferenceReason";
import type { PreferenceResolution } from "./bindings/PreferenceResolution";
import type { PreferenceState } from "./bindings/PreferenceState";
import type { PreferredSource } from "./bindings/PreferredSource";
import type { ProbeOutcome } from "./bindings/ProbeOutcome";
import type { RobloxDetection } from "./bindings/RobloxDetection";
import type { RobloxInstall } from "./bindings/RobloxInstall";

export type {
  DetectionSource,
  DetectionStatus,
  Finding,
  InstallChoice,
  InstallSelection,
  PreferenceReason,
  PreferenceResolution,
  PreferenceState,
  PreferredSource,
  ProbeOutcome,
  RobloxDetection,
  RobloxInstall,
};

/**
 * The Roblox service. Nothing here launches anything, and the page never sends a path or a
 * command: the only thing it can send is which kind of source to prefer.
 *
 * - `detectRoblox` only reads, and its answer is for the screen.
 * - `getInstallSelection` reads the same detection plus the saved preference, from one detection.
 * - `setPreferredInstall` saves the preference. The choice is a required argument, so leaving it
 *   out is a type error here and an error in the backend, never a silent reset to "automatic".
 */
export const robloxApi = {
  detectRoblox: () => invoke<RobloxDetection>("detect_roblox"),
  getInstallSelection: () => invoke<InstallSelection>("get_install_selection"),
  setPreferredInstall: (choice: InstallChoice) => invoke<InstallSelection>("set_preferred_install", { choice }),
};
