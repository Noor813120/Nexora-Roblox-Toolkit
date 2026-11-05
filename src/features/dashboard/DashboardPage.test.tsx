import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { backend, makeLog, mockTauri } from "../../test/mockTauri";
import { DashboardPage } from "./DashboardPage";

const card = (name: string) => screen.getByRole("region", { name });

describe("DashboardPage with a working backend", () => {
  it("shows real application, core count and memory values", async () => {
    mockTauri(backend());
    render(<DashboardPage />);

    expect(await screen.findByText("Running")).toBeInTheDocument();
    expect(within(card("Application")).getByText("Connected")).toBeInTheDocument();
    expect(within(card("Application")).getByText(/Version 0\.1\.0 · windows x86_64/)).toBeInTheDocument();

    expect(await within(card("CPU")).findByText("8 logical cores")).toBeInTheDocument();
    expect(await within(card("Memory")).findByText("10.0 GB of 16.0 GB")).toBeInTheDocument();
    const bar = within(card("Memory")).getByRole("progressbar", { name: "Computer memory in use" });
    expect(bar).toHaveAttribute("aria-valuenow", "63");
  });

  it("labels unimplemented metrics instead of inventing numbers", async () => {
    mockTauri(backend());
    render(<DashboardPage />);
    await screen.findByText("Running");

    const clients = card("Roblox clients");
    expect(within(clients).getByText("—")).toBeInTheDocument();
    expect(within(clients).getByText("Not available yet")).toBeInTheDocument();
    expect(within(clients).getByText(/not a count of zero/i)).toBeInTheDocument();

    expect(within(card("CPU")).getByText("Usage not measured yet")).toBeInTheDocument();
    expect(within(card("Automation")).getByText("Not available yet")).toBeInTheDocument();
    expect(await within(card("Recent activity")).findByText("No activity yet")).toBeInTheDocument();
    expect(within(card("Memory")).getByText(/Whole computer.*Not this app or Roblox/)).toBeInTheDocument();
  });

  it("offers quick links to every other section", async () => {
    mockTauri(backend());
    render(<DashboardPage />);
    const nav = card("Quick navigation");
    const links = within(nav).getAllByRole("link");
    expect(links).toHaveLength(7);
    expect(within(nav).getByRole("link", { name: /Game Launcher/ })).toHaveAttribute("href", "#/launcher");
    expect(within(nav).queryByRole("link", { name: /^Dashboard/ })).toBeNull();
  });

  it("refreshes only the system snapshot when Refresh is pressed", async () => {
    const invoke = mockTauri(backend());
    render(<DashboardPage />);
    await screen.findByText("10.0 GB of 16.0 GB");
    const calls = (cmd: string) => invoke.mock.calls.filter(([c]) => c === cmd).length;
    expect([calls("app_info"), calls("system_info")]).toEqual([1, 1]);

    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await screen.findByText("10.0 GB of 16.0 GB");
    expect([calls("app_info"), calls("system_info")]).toEqual([1, 2]);
  });
});

describe("DashboardPage when data is missing", () => {
  it("says memory is unavailable when the backend has no reading, and draws no bar", async () => {
    mockTauri(backend({ system_info: () => ({ logicalCores: 4, memory: null }) }));
    render(<DashboardPage />);
    expect(await within(card("Memory")).findByText("Unavailable")).toBeInTheDocument();
    expect(within(card("Memory")).getByText(/only implemented for Windows/)).toBeInTheDocument();
    expect(within(card("Memory")).queryByRole("progressbar")).toBeNull();
    expect(await within(card("CPU")).findByText("4 logical cores")).toBeInTheDocument();
  });

  it("uses the singular for one core", async () => {
    mockTauri(backend({ system_info: () => ({ logicalCores: 1, memory: null }) }));
    render(<DashboardPage />);
    expect(await within(card("CPU")).findByText("1 logical core")).toBeInTheDocument();
  });

  it("shows an offline notice, and disables Refresh, outside the app window", async () => {
    render(<DashboardPage />);
    expect(await within(card("Application")).findByText("Offline")).toBeInTheDocument();
    expect(within(card("Application")).getByText("Not connected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
  });

  it("shows the backend's error message and a Failed label", async () => {
    mockTauri(
      backend({
        app_info: () => {
          throw { code: "internal", message: "internal error" };
        },
      }),
    );
    render(<DashboardPage />);
    expect(await within(card("Application")).findByText("Failed")).toBeInTheDocument();
    expect(within(card("Application")).getByText("internal error")).toBeInTheDocument();
  });
});

describe("DashboardPage recent activity", () => {
  it("lists real entries from the database, newest first, with severity words", async () => {
    mockTauri(
      backend({
        list_activity_logs: () => ({
          items: [
            makeLog(3, { message: "Database backup saved (52 KB).", severity: "info" }),
            makeLog(2, { message: "Restore went wrong", severity: "error" }),
            makeLog(1, { message: "Nexora 0.1.0 started.", severity: "warning" }),
          ],
          nextBeforeId: null,
        }),
      }),
    );
    render(<DashboardPage />);
    const recent = card("Recent activity");
    expect(await within(recent).findByText("Database backup saved (52 KB).")).toBeInTheDocument();
    const items = within(recent).getAllByRole("listitem");
    expect(items).toHaveLength(3);
    const [first, second, third] = items as [HTMLElement, HTMLElement, HTMLElement];
    expect(within(first).getByText("Info")).toBeInTheDocument();
    expect(within(second).getByText("Error")).toBeInTheDocument();
    expect(within(third).getByText("Warning")).toBeInTheDocument();
    expect(within(recent).queryByText("No activity yet")).toBeNull();
    expect(within(recent).getByRole("link", { name: "View all activity" })).toHaveAttribute("href", "#/logs");
  });

  it("asks for as many entries as the saved setting says, in two calls", async () => {
    const invoke = mockTauri(
      backend({
        list_settings: () => [
          { key: "ui.dashboard_activity_count", value: "5", defaultValue: "8", isDefault: false, min: 3, max: 25, updatedAt: 1 },
        ],
      }),
    );
    render(<DashboardPage />);
    await within(card("Recent activity")).findByText("No activity yet");
    const logCalls = invoke.mock.calls.filter(([c]) => c === "list_activity_logs");
    expect(logCalls).toHaveLength(1);
    expect(logCalls[0]?.[1]).toEqual({ query: { limit: 5, beforeId: null, severity: null, eventType: null } });
    expect(invoke.mock.calls.filter(([c]) => c === "list_settings")).toHaveLength(1);
  });

  it("falls back to the default count when the setting cannot be read as a number", async () => {
    const invoke = mockTauri(backend({ list_settings: () => [] }));
    render(<DashboardPage />);
    await within(card("Recent activity")).findByText("No activity yet");
    const call = invoke.mock.calls.find(([c]) => c === "list_activity_logs");
    expect((call?.[1] as { query: { limit: number } }).query.limit).toBe(8);
  });

  it("shows the backend's error, with a retry that works", async () => {
    let fail = true;
    mockTauri(
      backend({
        list_activity_logs: () => {
          if (fail) throw { code: "unavailable", message: "the database is not available" };
          return { items: [makeLog(1, { message: "Back again" })], nextBeforeId: null };
        },
      }),
    );
    render(<DashboardPage />);
    const recent = card("Recent activity");
    expect(await within(recent).findByText("the database is not available")).toBeInTheDocument();
    fail = false;
    fireEvent.click(within(recent).getByRole("button", { name: "Try again" }));
    expect(await within(recent).findByText("Back again")).toBeInTheDocument();
  });

  it("shows an offline notice outside the app window", async () => {
    render(<DashboardPage />);
    expect(await within(card("Recent activity")).findByText(/only available inside/)).toBeInTheDocument();
  });

  it("Refresh reloads the activity list as well as the system snapshot", async () => {
    const invoke = mockTauri(backend());
    render(<DashboardPage />);
    await within(card("Recent activity")).findByText("No activity yet");
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(invoke.mock.calls.filter(([c]) => c === "list_activity_logs")).toHaveLength(2));
  });
});

describe("DashboardPage backend traffic", () => {
  it("makes four calls to show everything, then none while idle (nothing polls)", async () => {
    const invoke = mockTauri(backend());
    render(<DashboardPage />);
    await within(card("Recent activity")).findByText("No activity yet");
    await screen.findByText("10.0 GB of 16.0 GB");
    const names = () => invoke.mock.calls.map(([c]) => c).sort();
    expect(names()).toEqual(["app_info", "list_activity_logs", "list_settings", "system_info"]);

    vi.useFakeTimers();
    try {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(10 * 60 * 1000); // ten idle minutes
      });
    } finally {
      vi.useRealTimers();
    }
    expect(invoke).toHaveBeenCalledTimes(4);
  });
});
