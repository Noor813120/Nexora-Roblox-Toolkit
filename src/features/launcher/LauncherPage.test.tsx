
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  backend,
  mockTauri,
  ROBLOX_NOT_DETECTED,
  selection,
} from "../../test/mockTauri";
import { LauncherPage } from "./LauncherPage";

const NO_CLIENTS = {
  status: "available" as const,
  clients: [],
};

const ONE_CLIENT = {
  status: "available" as const,
  clients: [
    {
      processId: 1234,
      executableName: "RobloxPlayerBeta.exe",
    },
  ],
};

describe("LauncherPage", () => {
  it("shows the installation card and running-client empty state", async () => {
    const invoke = mockTauri(
      backend({
        get_install_selection: () => selection(ROBLOX_NOT_DETECTED),
        get_running_roblox_clients: () => NO_CLIENTS,
      }),
    );

    render(<LauncherPage />);

    expect(
      screen.getByRole("heading", {
        level: 1,
        name: "Game Launcher",
      }),
    ).toBeInTheDocument();

    expect(
      await screen.findByText("Not detected"),
    ).toBeInTheDocument();

    expect(
      await screen.findByText("No matching processes"),
    ).toBeInTheDocument();

    expect(
      screen.getByRole("heading", { name: "Running Roblox clients" }),
    ).toBeInTheDocument();

    expect(
      screen.getByText("Game targets and launch queue"),
    ).toBeInTheDocument();

    const commands = invoke.mock.calls.map(([command]) => command);

    expect(commands).toContain("get_install_selection");
    expect(commands).toContain("get_running_roblox_clients");
  });

  it("displays matching Roblox processes and their process IDs", async () => {
    mockTauri(
      backend({
        get_running_roblox_clients: () => ONE_CLIENT,
      }),
    );

    render(<LauncherPage />);

    expect(
      await screen.findByText("RobloxPlayerBeta.exe"),
    ).toBeInTheDocument();

    expect(screen.getByText("Process ID: 1234")).toBeInTheDocument();
    expect(screen.getByText("1 client detected")).toBeInTheDocument();
  });

  it("supports multiple detected clients", async () => {
    mockTauri(
      backend({
        get_running_roblox_clients: () => ({
          status: "available",
          clients: [
            { processId: 1234, executableName: "RobloxPlayerBeta.exe" },
            { processId: 5678, executableName: "RobloxPlayerBeta.exe" },
          ],
        }),
      }),
    );

    render(<LauncherPage />);

    expect(await screen.findByText("2 clients detected")).toBeInTheDocument();
    expect(screen.getByText("Process ID: 1234")).toBeInTheDocument();
    expect(screen.getByText("Process ID: 5678")).toBeInTheDocument();
  });

  it("shows an unsupported-platform state", async () => {
    mockTauri(
      backend({
        get_running_roblox_clients: () => ({
          status: "unsupported",
          clients: [],
        }),
      }),
    );

    render(<LauncherPage />);

    expect(
      await screen.findByText("Unsupported platform"),
    ).toBeInTheDocument();

    expect(
      screen.getByText(/available only on Windows/i),
    ).toBeInTheDocument();
  });

  it("shows an error instead of reporting zero clients when detection fails", async () => {
    mockTauri(
      backend({
        get_running_roblox_clients: () => {
          throw { code: "internal", message: "Snapshot failed." };
        },
      }),
    );

    render(<LauncherPage />);

    expect(
      await screen.findByRole("alert"),
    ).toHaveTextContent("Could not check running Roblox clients");

    expect(
      screen.queryByText("No matching processes"),
    ).not.toBeInTheDocument();
  });

  it("refreshes only when the user presses Refresh", async () => {
    let calls = 0;

    const invoke = mockTauri(
      backend({
        get_running_roblox_clients: () => {
          calls += 1;
          return calls === 1 ? ONE_CLIENT : NO_CLIENTS;
        },
      }),
    );

    render(<LauncherPage />);

    expect(
      await screen.findByText("Process ID: 1234"),
    ).toBeInTheDocument();

    const initialCalls = invoke.mock.calls.filter(
      ([command]) => command === "get_running_roblox_clients",
    ).length;

    expect(initialCalls).toBe(1);

    fireEvent.click(
      screen.getByRole("button", { name: "Refresh running clients" }),
    );

    expect(
      await screen.findByText("No matching processes"),
    ).toBeInTheDocument();

    await waitFor(() => {
      const refreshCalls = invoke.mock.calls.filter(
        ([command]) => command === "get_running_roblox_clients",
      ).length;

      expect(refreshCalls).toBe(2);
    });
  });

  it("does not write preferences or activity logs when loading the page", async () => {
    const invoke = mockTauri(
      backend({
        get_running_roblox_clients: () => NO_CLIENTS,
      }),
    );

    render(<LauncherPage />);

    await screen.findByText("No matching processes");
    await screen.findByText("Not detected");

    const commands = invoke.mock.calls.map(([command]) => String(command));

    expect(commands).not.toContain("set_preferred_install");
    expect(
      commands.some((command) => /activity|log|setting|profile/i.test(command)),
    ).toBe(false);
  });
});
