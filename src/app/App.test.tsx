import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { APP_INFO, mockTauri, SYSTEM_INFO } from "../test/mockTauri";
import { App } from "./App";
import { NAV_ITEMS } from "./navigation";

const goTo = (hash: string) =>
  act(async () => {
    window.location.hash = hash;
    await Promise.resolve();
  });

const h1 = (name: string) => screen.findByRole("heading", { level: 1, name });

describe("App shell", () => {
  it("opens on the Dashboard when there is no hash", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    render(<App />);
    expect(await h1("Dashboard")).toBeInTheDocument();
    expect(document.title).toBe("Dashboard · RoALT Advanced");
    expect(screen.getByRole("link", { name: "Dashboard" })).toHaveAttribute("aria-current", "page");
  });

  it("falls back to the Dashboard for an unknown hash", async () => {
    window.location.hash = "#/does-not-exist";
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    render(<App />);
    expect(await h1("Dashboard")).toBeInTheDocument();
  });

  it("renders every section's page with its own heading and a planned-phase label", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    render(<App />);
    await h1("Dashboard");
    for (const item of NAV_ITEMS.filter((i) => i.id !== "dashboard")) {
      await goTo(`#${item.path}`);
      expect(await h1(item.label)).toBeInTheDocument();
      expect(screen.getByRole("link", { name: item.label })).toHaveAttribute("aria-current", "page");
      expect(document.title).toBe(`${item.label} · RoALT Advanced`);
      if (item.id !== "settings") expect(screen.getByText(/Planned for Phase \d/)).toBeInTheDocument();
    }
  });

  it("moves keyboard focus to the page content on navigation", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    render(<App />);
    await h1("Dashboard");
    await goTo("#/accounts");
    await h1("Accounts");
    expect(screen.getByRole("main")).toHaveFocus();
  });

  it("shows About information from the backend on the Settings page", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    window.location.hash = "#/settings";
    render(<App />);
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText("x86_64")).toBeInTheDocument();
  });

  it("has a skip link that moves focus to the content", async () => {
    mockTauri({ app_info: () => APP_INFO, system_info: () => SYSTEM_INFO });
    render(<App />);
    await h1("Dashboard");
    screen.getByRole("button", { name: "Skip to content" }).click();
    expect(screen.getByRole("main")).toHaveFocus();
  });
});
