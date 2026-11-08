
import { describe, expect, it } from "vitest";
import { mockTauri } from "../test/mockTauri";
import { api } from "./index";

describe("running Roblox clients API", () => {
  it("calls get_running_roblox_clients without arguments", async () => {
    const result = {
      status: "available" as const,
      clients: [
        {
          processId: 1234,
          executableName: "RobloxPlayerBeta.exe",
        },
      ],
    };

    const invoke = mockTauri({
      get_running_roblox_clients: () => result,
    });

    await expect(api.getRunningRobloxClients()).resolves.toEqual(result);

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0]?.[0]).toBe("get_running_roblox_clients");
    expect(invoke.mock.calls[0]?.[1]).toEqual({});
  });

  it("passes backend errors through unchanged", async () => {
    const error = {
      code: "internal",
      message: "Something went wrong.",
    };

    mockTauri({
      get_running_roblox_clients: () => {
        throw error;
      },
    });

    await expect(api.getRunningRobloxClients()).rejects.toEqual(error);
  });

  it("does not accept a process ID, path, or command as input", () => {
    expect(api.getRunningRobloxClients.length).toBe(0);
  });
});
