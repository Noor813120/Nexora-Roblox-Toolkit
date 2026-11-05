import { invoke } from "@tauri-apps/api/core";
import type { ActivityLog } from "./bindings/ActivityLog";
import type { ActivityLogPage } from "./bindings/ActivityLogPage";
import type { ActivityLogQuery } from "./bindings/ActivityLogQuery";
import type { AutomationProfile } from "./bindings/AutomationProfile";
import type { AutomationProfileInput } from "./bindings/AutomationProfileInput";
import type { BackupOutcome } from "./bindings/BackupOutcome";
import type { DatabaseStatus } from "./bindings/DatabaseStatus";
import type { NewActivityLog } from "./bindings/NewActivityLog";
import type { Profile } from "./bindings/Profile";
import type { ProfileInput } from "./bindings/ProfileInput";
import type { RestoreOutcome } from "./bindings/RestoreOutcome";
import type { SettingEntry } from "./bindings/SettingEntry";
import type { Severity } from "./bindings/Severity";

export type {
  ActivityLog,
  ActivityLogPage,
  ActivityLogQuery,
  AutomationProfile,
  AutomationProfileInput,
  BackupOutcome,
  DatabaseStatus,
  NewActivityLog,
  Profile,
  ProfileInput,
  RestoreOutcome,
  SettingEntry,
  Severity,
};

/** The query with every field optional; the rest are filled in as `null` ("not set"). */
export type ActivityQueryOptions = Partial<ActivityLogQuery>;

/**
 * The database service: one typed function per backend command. The React side never builds SQL
 * and never passes a file path; backup and restore open the operating system's file dialog from
 * Rust. Argument names are camelCase because Tauri converts the Rust `snake_case` names.
 */
export const databaseApi = {
  databaseStatus: () => invoke<DatabaseStatus>("database_status"),

  getSetting: (key: string) => invoke<SettingEntry>("get_setting", { key }),
  listSettings: () => invoke<SettingEntry[]>("list_settings"),
  setSetting: (key: string, value: string) => invoke<SettingEntry>("set_setting", { key, value }),
  /** Resets the setting to its default and returns the effective entry. */
  deleteSetting: (key: string) => invoke<SettingEntry>("delete_setting", { key }),

  listProfiles: () => invoke<Profile[]>("list_profiles"),
  createProfile: (input: ProfileInput) => invoke<Profile>("create_profile", { input }),
  updateProfile: (id: number, input: ProfileInput) => invoke<Profile>("update_profile", { id, input }),
  deleteProfile: (id: number) => invoke<void>("delete_profile", { id }),

  listAutomationProfiles: () => invoke<AutomationProfile[]>("list_automation_profiles"),
  createAutomationProfile: (input: AutomationProfileInput) =>
    invoke<AutomationProfile>("create_automation_profile", { input }),
  updateAutomationProfile: (id: number, input: AutomationProfileInput) =>
    invoke<AutomationProfile>("update_automation_profile", { id, input }),
  deleteAutomationProfile: (id: number) => invoke<void>("delete_automation_profile", { id }),

  listActivityLogs: (options: ActivityQueryOptions = {}) => {
    const query: ActivityLogQuery = { limit: null, beforeId: null, severity: null, eventType: null, ...options };
    return invoke<ActivityLogPage>("list_activity_logs", { query });
  },
  createActivityLog: (entry: NewActivityLog) => invoke<ActivityLog>("create_activity_log", { entry }),
  clearOldActivityLogs: (olderThanDays: number) => invoke<number>("clear_old_activity_logs", { olderThanDays }),

  backupDatabase: () => invoke<BackupOutcome>("backup_database"),
  restoreDatabase: () => invoke<RestoreOutcome>("restore_database"),
};
