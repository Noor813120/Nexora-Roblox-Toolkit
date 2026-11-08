
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
import type { ProcessScanStatus } from "./bindings/ProcessScanStatus";
import type { RobloxDetection } from "./bindings/RobloxDetection";
import type { RobloxInstall } from "./bindings/RobloxInstall";
import type { RunningRobloxClient } from "./bindings/RunningRobloxClient";
import type { RunningRobloxClients } from "./bindings/RunningRobloxClients";

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
  ProcessScanStatus,
  RobloxDetection,
  RobloxInstall,
  RunningRobloxClient,
  RunningRobloxClients,
};

/**
 * The Roblox service. Installation detection and process discovery are read-only.
 *
 * - detectRoblox reads installed Roblox locations.
 * - getInstallSelection reads detection and the saved installation preference.
 * - setPreferredInstall saves the user's explicitly selected preference.
 * - getRunningRobloxClients takes one process snapshot. It does not launch or
 *   modify any process.
 */
export const robloxApi = {
  detectRoblox: () => invoke<RobloxDetection>("detect_roblox"),
  getInstallSelection: () => invoke<InstallSelection>("get_install_selection"),
  setPreferredInstall: (choice: InstallChoice) =>
    invoke<InstallSelection>("set_preferred_install", { choice }),
  getRunningRobloxClients: () =>
    invoke<RunningRobloxClients>("get_running_roblox_clients"),
};
