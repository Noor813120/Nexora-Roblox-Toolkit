import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { APP_INFO, mockTauri, SYSTEM_INFO } from "../../test/mockTauri";
import { DashboardPage } from "./DashboardPage";

const card = (name: string) => screen.getByRole("region", { name });

describe("DashboardPage with a working backend", () => {
  it("shows real application, core count and memory values", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
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
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    render(<DashboardPage />);
    await screen.findByText("Running");

    const clients = card("Roblox clients");
    expect(within(clients).getByText("—")).toBeInTheDocument();
    expect(within(clients).getByText("Not available yet")).toBeInTheDocument();
    expect(within(clients).getByText(/not a count of zero/i)).toBeInTheDocument();

    expect(within(card("CPU")).getByText("Usage not measured yet")).toBeInTheDocument();
    expect(within(card("Automation")).getByText("Not available yet")).toBeInTheDocument();
    expect(within(card("Recent activity")).getByText("No activity yet")).toBeInTheDocument();
    expect(within(card("Memory")).getByText(/Whole computer.*Not this app or Roblox/)).toBeInTheDocument();
  });

  it("offers quick links to every other section", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    render(<DashboardPage />);
    const nav = card("Quick navigation");
    const links = within(nav).getAllByRole("link");
    expect(links).toHaveLength(7);
    expect(within(nav).getByRole("link", { name: /Game Launcher/ })).toHaveAttribute("href", "#/launcher");
    expect(within(nav).queryByRole("link", { name: /^Dashboard/ })).toBeNull();
  });

  it("refreshes only the system snapshot when Refresh is pressed", async () => {
    const invoke = mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
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
    mockTauri({ app_info: () => APP_INFO, system_info: () => ({ logicalCores: 4, memory: null }) });
    render(<DashboardPage />);
    expect(await within(card("Memory")).findByText("Unavailable")).toBeInTheDocument();
    expect(within(card("Memory")).getByText(/only implemented for Windows/)).toBeInTheDocument();
    expect(within(card("Memory")).queryByRole("progressbar")).toBeNull();
    expect(await within(card("CPU")).findByText("4 logical cores")).toBeInTheDocument();
  });

  it("uses the singular for one core", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => ({ logicalCores: 1, memory: null }) });
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
    mockTauri({
      app_info: () => {
        throw { code: "internal", message: "internal error" };
      },
      system_info: () => SYSTEM_INFO,
    });
    render(<DashboardPage />);
    expect(await within(card("Application")).findByText("Failed")).toBeInTheDocument();
    expect(within(card("Application")).getByText("internal error")).toBeInTheDocument();
  });
});
