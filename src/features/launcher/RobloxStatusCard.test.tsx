import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  mockTauri,
  ROBLOX_DETECTED,
  ROBLOX_INCONCLUSIVE,
  ROBLOX_INCONCLUSIVE_INCOMPLETE,
  ROBLOX_INCONCLUSIVE_MALFORMED,
  ROBLOX_NOT_DETECTED,
  ROBLOX_UNSUPPORTED,
} from "../../test/mockTauri";
import { RobloxStatusCard } from "./RobloxStatusCard";

const card = () => screen.getByRole("region", { name: "Roblox installation" });

describe("RobloxStatusCard", () => {
  it("shows a loading state, then an install with where it came from", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_DETECTED });
    render(<RobloxStatusCard />);
    expect(within(card()).getByText("Looking for Roblox…")).toBeInTheDocument();

    expect(await within(card()).findByText("Roblox files found")).toBeInTheDocument();
    const installs = within(card()).getByRole("list", { name: "Roblox installs found" });
    const items = within(installs).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(within(items[0]!).getByText("Windows link handler")).toBeInTheDocument();
    expect(within(items[0]!).getByText("Primary")).toBeInTheDocument();
    expect(within(items[0]!).getByText(/RobloxPlayerLauncher\.exe$/)).toBeInTheDocument();
    expect(within(items[0]!).getAllByText("version-0123456789abcdef").length).toBeGreaterThan(0);
    expect(within(items[0]!).getByText("Last updated")).toBeInTheDocument();
    expect(within(items[1]!).getByText("Roblox version folders")).toBeInTheDocument();
    expect(within(items[1]!).queryByText("Primary")).not.toBeInTheDocument();
    expect(within(items[1]!).queryByText("Last updated")).not.toBeInTheDocument(); // date unknown: not invented
  });

  it("says plainly that nothing was found, and that Microsoft Store installs are not checked", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_NOT_DETECTED });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Not detected")).toBeInTheDocument();
    expect(within(card()).queryByRole("list", { name: "Roblox installs found" })).not.toBeInTheDocument();

    const checked = within(card()).getByRole("list", { name: "What was checked" });
    expect(within(checked).getAllByRole("listitem")).toHaveLength(3);
    const store = within(checked).getByText("Microsoft Store").closest("li")!;
    expect(within(store).getByText("Not checked")).toBeInTheDocument();
    expect(within(store).getByText(/Microsoft Store installs are not checked/)).toBeInTheDocument();
  });

  it("does not present an unreadable place as 'not installed'", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_INCONCLUSIVE });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Could not tell")).toBeInTheDocument();
    expect(within(card()).queryByText("Not detected")).not.toBeInTheDocument();
    expect(within(card()).getByText(/may still be installed/)).toBeInTheDocument();
    expect(within(card()).getByText("Could not check")).toBeInTheDocument();
  });

  it("does not claim every location was empty when nothing was detected", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_NOT_DETECTED });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Not detected");
    expect(
      within(card()).getByText(
        "No supported Roblox installation location could be verified. Some installation types, including Microsoft Store installations, are not checked.",
      ),
    ).toBeInTheDocument();
    expect(within(card()).queryByText(/every place/i)).not.toBeInTheDocument();
    expect(within(card()).queryByText(/came back empty/i)).not.toBeInTheDocument();
  });

  it("describes found files as passing basic checks, not as genuine Roblox", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_DETECTED });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    expect(within(card()).getAllByText("Passed basic checks").length).toBeGreaterThan(0);
    expect(within(card()).getByText(/genuine Roblox software was not checked/)).toBeInTheDocument();
    expect(within(card()).getByText(/Nexora did not start Roblox/)).toBeInTheDocument();
    expect(card().textContent).not.toMatch(/\bverified\b|\bvalid\b/i);
  });

  it("shows a malformed handler as 'Could not tell', with hidden problems as 'Several problems'", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_INCONCLUSIVE_MALFORMED });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Could not tell")).toBeInTheDocument();
    expect(within(card()).queryByText("Not detected")).not.toBeInTheDocument();
    const checked = within(card()).getByRole("list", { name: "What was checked" });
    expect(within(checked).getByText("Ignored")).toBeInTheDocument();
    expect(within(checked).getByText("Several problems")).toBeInTheDocument();
  });

  it("explains an unsupported system", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_UNSUPPORTED });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Not supported here")).toBeInTheDocument();
    expect(within(card()).getByText("Roblox detection is only available on Windows.")).toBeInTheDocument();
  });

  it("shows backend errors as an alert, with no result", async () => {
    mockTauri({
      detect_roblox: () => {
        throw { code: "internal", message: "Something went wrong." };
      },
    });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByRole("alert")).toHaveTextContent("Something went wrong.");
    expect(within(card()).queryByText("Roblox files found")).not.toBeInTheDocument();
    expect(within(card()).queryByText("Not detected")).not.toBeInTheDocument();
  });

  it("is offline, and disabled, outside the app window", async () => {
    render(<RobloxStatusCard />); // no mockTauri: not inside Tauri
    expect(await within(card()).findByText(/only works inside the Nexora window/)).toBeInTheDocument();
    expect(within(card()).getByRole("button", { name: "Check again" })).toBeDisabled();
  });

  it("checks again on request and shows the new answer", async () => {
    let answer: unknown = ROBLOX_NOT_DETECTED;
    const invoke = mockTauri({ detect_roblox: () => answer });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Not detected")).toBeInTheDocument();

    answer = ROBLOX_DETECTED;
    fireEvent.click(within(card()).getByRole("button", { name: "Check again" }));
    expect(await within(card()).findByText("Roblox files found")).toBeInTheDocument();
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    expect(invoke.mock.calls.every(([cmd]) => cmd === "detect_roblox")).toBe(true);
  });

  it("shows a partly listed folder as 'Could not tell' with 'Partly checked'", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_INCONCLUSIVE_INCOMPLETE });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Could not tell")).toBeInTheDocument();
    expect(within(card()).queryByText("Not detected")).not.toBeInTheDocument();
    const checked = within(card()).getByRole("list", { name: "What was checked" });
    expect(within(checked).getByText("Partly checked")).toBeInTheDocument();
    expect(within(checked).getByText(/may have been missed/)).toBeInTheDocument();
  });

  it("shows 'Checking…', disables the button and sends one request while a check runs", async () => {
    let finish: (value: unknown) => void = () => {};
    let calls = 0;
    const invoke = mockTauri({
      detect_roblox: () => {
        calls += 1;
        return calls === 1 ? ROBLOX_NOT_DETECTED : new Promise((resolve) => (finish = resolve));
      },
    });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Not detected");

    const button = within(card()).getByRole("button", { name: "Check again" });
    fireEvent.click(button);
    const busy = await within(card()).findByRole("button", { name: "Checking…" });
    expect(busy).toBeDisabled();
    expect(within(card()).getByText("Checking for Roblox again…")).toBeInTheDocument();
    expect(within(card()).getByText("Not detected")).toBeInTheDocument(); // old answer stays visible
    fireEvent.click(busy);
    fireEvent.click(busy);
    expect(invoke).toHaveBeenCalledTimes(2); // first load + one refresh, not three

    finish(ROBLOX_DETECTED);
    expect(await within(card()).findByText("Roblox files found")).toBeInTheDocument();
    expect(within(card()).getByRole("button", { name: "Check again" })).toBeEnabled();
    expect(within(card()).queryByText("Checking for Roblox again…")).not.toBeInTheDocument();
  });

  it("keeps one live region mounted from the first render, so updates are announced", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_DETECTED });
    const { container } = render(<RobloxStatusCard />);
    const region = container.querySelector('[aria-live="polite"]');
    expect(region).not.toBeNull(); // present before any result exists
    await within(card()).findByText("Roblox files found");
    expect(container.querySelector('[aria-live="polite"]')).toBe(region); // same element, not remounted
    expect(region).toHaveTextContent("Roblox files found");
  });

  it("only offers 'Check again': nothing here can launch or change anything", async () => {
    mockTauri({ detect_roblox: () => ROBLOX_DETECTED });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    const buttons = within(card()).getAllByRole("button");
    expect(buttons.map((b) => b.textContent)).toEqual(["Check again"]);
    expect(within(card()).queryByRole("link")).not.toBeInTheDocument();
  });
});
