import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend, mockTauri, ROBLOX_NOT_DETECTED } from "../../test/mockTauri";
import { LauncherPage } from "./LauncherPage";

describe("LauncherPage", () => {
  it("shows the Roblox installation card and keeps the existing 'planned' content", async () => {
    const invoke = mockTauri(backend({ detect_roblox: () => ROBLOX_NOT_DETECTED }));
    render(<LauncherPage />);

    expect(screen.getByRole("heading", { level: 1, name: "Game Launcher" })).toBeInTheDocument();
    const card = screen.getByRole("region", { name: "Roblox installation" });
    expect(await within(card).findByText("Not detected")).toBeInTheDocument();

    // Launching and client tracking are still not built, and the page still says so.
    expect(screen.getByRole("heading", { name: "No clients running" })).toBeInTheDocument();
    expect(screen.getByText("Planned for Phase 4")).toBeInTheDocument();

    // The page reads one thing, once, and nothing else.
    expect(invoke.mock.calls.map(([cmd]) => cmd)).toEqual(["detect_roblox"]);
  });

  it("never writes detection to the activity log", async () => {
    const invoke = mockTauri(backend());
    render(<LauncherPage />);
    await screen.findByText("Not detected");
    const commands = invoke.mock.calls.map(([cmd]) => String(cmd));
    expect(commands.some((c) => /activity|log|setting|profile/i.test(c))).toBe(false);
  });
});
