import { describe, expect, it } from "vitest";
import { mockTauri } from "../test/mockTauri";
import { api } from "./index";

/** Every database command, the exact name and argument shape Rust expects. */
describe("database service", () => {
  const calls: Array<[string, () => Promise<unknown>, string, unknown]> = [
    ["databaseStatus", () => api.databaseStatus(), "database_status", {}],
    ["getSetting", () => api.getSetting("a.b"), "get_setting", { key: "a.b" }],
    ["listSettings", () => api.listSettings(), "list_settings", {}],
    ["setSetting", () => api.setSetting("a.b", "5"), "set_setting", { key: "a.b", value: "5" }],
    ["deleteSetting", () => api.deleteSetting("a.b"), "delete_setting", { key: "a.b" }],
    ["listProfiles", () => api.listProfiles(), "list_profiles", {}],
    [
      "createProfile",
      () => api.createProfile({ name: "N", description: "", configurationJson: "{}", isDefault: false }),
      "create_profile",
      { input: { name: "N", description: "", configurationJson: "{}", isDefault: false } },
    ],
    [
      "updateProfile",
      () => api.updateProfile(7, { name: "N", description: "", configurationJson: "{}", isDefault: true }),
      "update_profile",
      { id: 7, input: { name: "N", description: "", configurationJson: "{}", isDefault: true } },
    ],
    ["deleteProfile", () => api.deleteProfile(7), "delete_profile", { id: 7 }],
    ["listAutomationProfiles", () => api.listAutomationProfiles(), "list_automation_profiles", {}],
    [
      "createAutomationProfile",
      () => api.createAutomationProfile({ name: "A", configurationJson: "{}", enabled: false }),
      "create_automation_profile",
      { input: { name: "A", configurationJson: "{}", enabled: false } },
    ],
    [
      "updateAutomationProfile",
      () => api.updateAutomationProfile(3, { name: "A", configurationJson: "{}", enabled: true }),
      "update_automation_profile",
      { id: 3, input: { name: "A", configurationJson: "{}", enabled: true } },
    ],
    ["deleteAutomationProfile", () => api.deleteAutomationProfile(3), "delete_automation_profile", { id: 3 }],
    [
      "createActivityLog",
      () => api.createActivityLog({ eventType: "a.b", severity: "info", message: "m", metadataJson: null }),
      "create_activity_log",
      { entry: { eventType: "a.b", severity: "info", message: "m", metadataJson: null } },
    ],
    ["clearOldActivityLogs", () => api.clearOldActivityLogs(30), "clear_old_activity_logs", { olderThanDays: 30 }],
    ["backupDatabase", () => api.backupDatabase(), "backup_database", {}],
    ["restoreDatabase", () => api.restoreDatabase(), "restore_database", {}],
  ];

  it.each(calls)("%s calls %s with the right arguments", async (_name, call, command, args) => {
    const invoke = mockTauri({ [command]: () => "ok" });
    await call();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0]?.[0]).toBe(command);
    expect(invoke.mock.calls[0]?.[1]).toEqual(args);
  });

  it("fills unset activity-log query fields with null, so Rust sees a complete query", async () => {
    const invoke = mockTauri({ list_activity_logs: () => ({ items: [], nextBeforeId: null }) });
    await api.listActivityLogs();
    await api.listActivityLogs({ limit: 10, severity: "error" });
    expect(invoke.mock.calls[0]?.[1]).toEqual({ query: { limit: null, beforeId: null, severity: null, eventType: null } });
    expect(invoke.mock.calls[1]?.[1]).toEqual({ query: { limit: 10, beforeId: null, severity: "error", eventType: null } });
  });

  it("passes backend errors through untouched", async () => {
    mockTauri({
      create_profile: () => {
        throw { code: "conflict", message: 'a profile named "Main" already exists' };
      },
    });
    await expect(
      api.createProfile({ name: "Main", description: "", configurationJson: "{}", isDefault: false }),
    ).rejects.toEqual({ code: "conflict", message: 'a profile named "Main" already exists' });
  });

  it("has no function that accepts SQL or a file path", () => {
    for (const [name, fn] of Object.entries(api)) {
      if (["backupDatabase", "restoreDatabase", "databaseStatus"].includes(name)) expect(fn.length).toBe(0);
    }
    expect(Object.keys(api).some((k) => /sql|query$|execute|rawPath/i.test(k))).toBe(false);
  });
});

describe("roblox service", () => {
  it("detectRoblox calls detect_roblox with no arguments", async () => {
    const detection = { status: "notDetected", installs: [], findings: [] };
    const invoke = mockTauri({ detect_roblox: () => detection });
    await expect(api.detectRoblox()).resolves.toEqual(detection);
    expect(invoke.mock.calls[0]?.[0]).toBe("detect_roblox");
    expect(invoke.mock.calls[0]?.[1]).toEqual({});
  });

  it("detection takes no input, so the page cannot choose a path or command", () => {
    expect(api.detectRoblox.length).toBe(0);
    expect(Object.keys(api).filter((k) => /roblox/i.test(k))).toEqual(["detectRoblox"]);
  });

  it("passes backend errors through untouched", async () => {
    mockTauri({
      detect_roblox: () => {
        throw { code: "internal", message: "Something went wrong." };
      },
    });
    await expect(api.detectRoblox()).rejects.toEqual({ code: "internal", message: "Something went wrong." });
  });
});
