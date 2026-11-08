import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  BOTH_SOURCES,
  mockTauri,
  PREF_AUTOMATIC,
  PREF_AVAILABLE_FOLDERS,
  ROBLOX_DETECTED,
  ROBLOX_INCONCLUSIVE,
  ROBLOX_INCONCLUSIVE_INCOMPLETE,
  ROBLOX_INCONCLUSIVE_MALFORMED,
  ROBLOX_NOT_DETECTED,
  ROBLOX_UNSUPPORTED,
  selection,
} from "../../test/mockTauri";
import { RobloxStatusCard } from "./RobloxStatusCard";

const card = () => screen.getByRole("region", { name: "Roblox installation" });

describe("RobloxStatusCard", () => {
  it("shows a loading state, then an install with where it came from", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED) });
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
    mockTauri({ get_install_selection: () => selection(ROBLOX_NOT_DETECTED) });
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
    mockTauri({ get_install_selection: () => selection(ROBLOX_INCONCLUSIVE) });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Could not tell")).toBeInTheDocument();
    expect(within(card()).queryByText("Not detected")).not.toBeInTheDocument();
    expect(within(card()).getByText(/may still be installed/)).toBeInTheDocument();
    expect(within(card()).getByText("Could not check")).toBeInTheDocument();
  });

  it("does not claim every location was empty when nothing was detected", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_NOT_DETECTED) });
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
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED) });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    expect(within(card()).getAllByText("Passed basic checks").length).toBeGreaterThan(0);
    expect(within(card()).getByText(/genuine Roblox software was not checked/)).toBeInTheDocument();
    expect(within(card()).getByText(/Nexora did not start Roblox/)).toBeInTheDocument();
    expect(card().textContent).not.toMatch(/\bverified\b|\bvalid\b/i);
  });

  it("shows a malformed handler as 'Could not tell', with hidden problems as 'Several problems'", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_INCONCLUSIVE_MALFORMED) });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Could not tell")).toBeInTheDocument();
    expect(within(card()).queryByText("Not detected")).not.toBeInTheDocument();
    const checked = within(card()).getByRole("list", { name: "What was checked" });
    expect(within(checked).getByText("Ignored")).toBeInTheDocument();
    expect(within(checked).getByText("Several problems")).toBeInTheDocument();
  });

  it("explains an unsupported system", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_UNSUPPORTED) });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Not supported here")).toBeInTheDocument();
    expect(within(card()).getByText("Roblox detection is only available on Windows.")).toBeInTheDocument();
  });

  it("shows backend errors as an alert, with no result", async () => {
    mockTauri({
      get_install_selection: () => {
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
    const button = within(card()).getByRole("button", { name: "Check again" });
    expect(button).toBeDisabled(); // genuinely disabled: there is nothing to check here
    expect(button).not.toHaveAttribute("aria-disabled");
  });

  it("checks again on request and shows the new answer", async () => {
    let answer: unknown = ROBLOX_NOT_DETECTED;
    const invoke = mockTauri({ get_install_selection: () => selection(answer) });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Not detected")).toBeInTheDocument();

    answer = ROBLOX_DETECTED;
    fireEvent.click(within(card()).getByRole("button", { name: "Check again" }));
    expect(await within(card()).findByText("Roblox files found")).toBeInTheDocument();
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    expect(invoke.mock.calls.every(([cmd]) => cmd === "get_install_selection")).toBe(true);
  });

  it("shows a partly listed folder as 'Could not tell' with 'Partly checked'", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_INCONCLUSIVE_INCOMPLETE) });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Could not tell")).toBeInTheDocument();
    expect(within(card()).queryByText("Not detected")).not.toBeInTheDocument();
    const checked = within(card()).getByRole("list", { name: "What was checked" });
    expect(within(checked).getByText("Partly checked")).toBeInTheDocument();
    expect(within(checked).getByText(/may have been missed/)).toBeInTheDocument();
  });

  it("shows 'Checking…', keeps the button focusable but inert, and sends one request while a check runs", async () => {
    let finish: (value: unknown) => void = () => {};
    let calls = 0;
    const invoke = mockTauri({
      get_install_selection: () => {
        calls += 1;
        return calls === 1 ? selection(ROBLOX_NOT_DETECTED) : new Promise((resolve) => (finish = resolve));
      },
    });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Not detected");

    const button = within(card()).getByRole("button", { name: "Check again" });
    fireEvent.click(button);
    const busy = await within(card()).findByRole("button", { name: "Checking…" });
    expect(busy).toHaveAttribute("aria-disabled", "true"); // announced as unavailable...
    expect(busy).toBeEnabled(); // ...but not disabled, so keyboard focus is not dropped
    expect(within(card()).getByText("Checking for Roblox again…")).toBeInTheDocument();
    expect(within(card()).getByText("Not detected")).toBeInTheDocument(); // old answer stays visible
    fireEvent.click(busy);
    fireEvent.click(busy);
    expect(invoke).toHaveBeenCalledTimes(2); // first load + one refresh, not three

    finish(selection(ROBLOX_DETECTED));
    expect(await within(card()).findByText("Roblox files found")).toBeInTheDocument();
    const again = within(card()).getByRole("button", { name: "Check again" });
    expect(again).toBeEnabled();
    expect(again).not.toHaveAttribute("aria-disabled");
    expect(within(card()).queryByText("Checking for Roblox again…")).not.toBeInTheDocument();
  });

  // Limit: jsdom does not move focus away from a button that becomes `disabled`, so this cannot
  // catch a return to `disabled`; the `aria-disabled` assertions above do. It does catch the
  // button being replaced (which would drop focus in a real browser).
  it("keeps keyboard focus on 'Check again' through a whole check", async () => {
    let finish: (value: unknown) => void = () => {};
    let calls = 0;
    mockTauri({
      get_install_selection: () => {
        calls += 1;
        return calls === 1 ? selection(ROBLOX_NOT_DETECTED) : new Promise((resolve) => (finish = resolve));
      },
    });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Not detected");

    const button = within(card()).getByRole("button", { name: "Check again" });
    button.focus();
    expect(button).toHaveFocus();
    fireEvent.click(button);
    const busy = await within(card()).findByRole("button", { name: "Checking…" });
    expect(busy).toBe(button); // the same element, not replaced
    expect(busy).toHaveFocus();

    finish(selection(ROBLOX_DETECTED));
    await within(card()).findByText("Roblox files found");
    const again = within(card()).getByRole("button", { name: "Check again" });
    expect(again).toBe(button);
    expect(again).toHaveFocus();
  });

  it("keeps one live region mounted from the first render, so updates are announced", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED) });
    const { container } = render(<RobloxStatusCard />);
    const region = container.querySelector('[aria-live="polite"]');
    expect(region).not.toBeNull(); // present before any result exists
    await within(card()).findByText("Roblox files found");
    expect(container.querySelector('[aria-live="polite"]')).toBe(region); // same element, not remounted
    expect(region).toHaveTextContent("Roblox files found");
  });

  it("only offers 'Check again' and 'Save choice': nothing here can launch anything", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED) });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    const buttons = within(card()).getAllByRole("button");
    expect(buttons.map((b) => b.textContent)).toEqual(["Check again", "Save choice"]);
    expect(within(card()).queryByRole("link")).not.toBeInTheDocument();
  });
});

describe("RobloxStatusCard: preferred installation", () => {
  it("shows the preference below the detection result, from the same answer", async () => {
    const invoke = mockTauri({
      get_install_selection: () => selection(ROBLOX_DETECTED, PREF_AVAILABLE_FOLDERS, BOTH_SOURCES),
    });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Roblox files found")).toBeInTheDocument();
    expect(within(card()).getByRole("group", { name: "Preferred installation" })).toBeInTheDocument();
    expect(within(card()).getByText(/using 1 of 2 installations found/)).toBeInTheDocument();
    expect(within(card()).getByRole("radio", { name: "Roblox version folders" })).toBeChecked();
    // One read, nothing written.
    expect(invoke.mock.calls.map(([cmd]) => cmd)).toEqual(["get_install_selection"]);
  });

  it("places the preference after the installs and before 'What was checked'", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED, PREF_AUTOMATIC, BOTH_SOURCES) });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    const installs = within(card()).getByRole("list", { name: "Roblox installs found" });
    const preference = within(card()).getByRole("group", { name: "Preferred installation" });
    const checked = within(card()).getByRole("heading", { name: "What was checked" });
    const comesBefore = (a: Node, b: Node) => Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
    expect(comesBefore(installs, preference)).toBe(true);
    expect(comesBefore(preference, checked)).toBe(true);
  });

  it("has two separate live regions that announce different things, and both survive a refresh", async () => {
    const invoke = mockTauri({
      get_install_selection: () => selection(ROBLOX_DETECTED, PREF_AUTOMATIC, BOTH_SOURCES),
    });
    const { container } = render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    const regions = () => Array.from(container.querySelectorAll('[aria-live="polite"]')) as HTMLElement[];

    expect(regions()).toHaveLength(2);
    const [cardRegion, panelRegion] = regions() as [HTMLElement, HTMLElement];
    // Neither is inside the other, so nothing is announced as part of both.
    expect(cardRegion.contains(panelRegion)).toBe(false);
    expect(panelRegion.contains(cardRegion)).toBe(false);
    // Each carries its own message, and neither repeats the other's.
    expect(cardRegion).toHaveTextContent("Roblox files found");
    expect(cardRegion).not.toHaveTextContent("Using the first installation found");
    expect(panelRegion).toHaveTextContent("Using the first installation found");
    expect(panelRegion).not.toHaveTextContent("Roblox files found");

    // They are the same elements after "Check again", not replaced.
    fireEvent.click(within(card()).getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(within(card()).getByRole("button", { name: "Check again" })).toBeEnabled());
    expect(regions()).toHaveLength(2);
    expect(regions()[0]).toBe(cardRegion);
    expect(regions()[1]).toBe(panelRegion);
  });

  it("an unreadable preference store does not hide detection, and is not shown as 'Automatic'", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED, null, BOTH_SOURCES) });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByText("Roblox files found")).toBeInTheDocument(); // detection is intact
    expect(within(card()).getByText(/Preference storage is unavailable/)).toBeInTheDocument();
    expect(within(card()).getByRole("group", { name: "Preferred installation" })).toBeDisabled();
    expect(within(card()).getByRole("button", { name: "Save choice" })).toBeDisabled();
    expect(within(card()).getByRole("button", { name: "Check again" })).toBeEnabled(); // M1 behaviour intact
  });

  it("saving reloads the whole selection and shows what the backend now says", async () => {
    let saved = false;
    const invoke = mockTauri({
      get_install_selection: () =>
        saved
          ? selection(ROBLOX_DETECTED, PREF_AVAILABLE_FOLDERS, BOTH_SOURCES)
          : selection(ROBLOX_DETECTED, PREF_AUTOMATIC, BOTH_SOURCES),
      set_preferred_install: () => {
        saved = true;
        // What this call returns is deliberately NOT what the next read says, to show that the
        // screen follows the reload and not the reply.
        return selection(ROBLOX_NOT_DETECTED, null, []);
      },
    });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    expect(within(card()).getByRole("radio", { name: "Automatic" })).toBeChecked();

    fireEvent.click(within(card()).getByRole("radio", { name: "Roblox version folders" }));
    fireEvent.click(within(card()).getByRole("button", { name: "Save choice" }));

    await waitFor(() => expect(within(card()).getByText(/using 1 of 2 installations found/)).toBeInTheDocument());
    expect(invoke.mock.calls.map(([cmd, args]) => [cmd, args])).toEqual([
      ["get_install_selection", {}],
      ["set_preferred_install", { choice: "versionFolder" }],
      ["get_install_selection", {}],
    ]);
    expect(within(card()).getByText("Roblox files found")).toBeInTheDocument(); // not the reply's detection
    expect(within(card()).getByRole("radio", { name: "Roblox version folders" })).toBeChecked();
  });

  it("'Check again' re-reads without writing, and keeps an unsaved pick", async () => {
    const invoke = mockTauri({
      get_install_selection: () => selection(ROBLOX_DETECTED, PREF_AUTOMATIC, BOTH_SOURCES),
    });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    fireEvent.click(within(card()).getByRole("radio", { name: "Windows link handler" }));
    fireEvent.click(within(card()).getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    expect(invoke.mock.calls.every(([cmd]) => cmd === "get_install_selection")).toBe(true);
    await waitFor(() => expect(within(card()).getByRole("button", { name: "Check again" })).toBeEnabled());
    expect(within(card()).getByRole("radio", { name: "Windows link handler" })).toBeChecked();
  });

  it("shows no preference control when offline or after an error", async () => {
    // Offline: not inside the app window.
    const { unmount } = render(<RobloxStatusCard />);
    expect(await within(card()).findByText(/only works inside the Nexora window/)).toBeInTheDocument();
    expect(within(card()).queryByRole("group", { name: "Preferred installation" })).not.toBeInTheDocument();
    unmount();

    // Error: the read failed.
    mockTauri({
      get_install_selection: () => {
        throw { code: "internal", message: "Something went wrong." };
      },
    });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByRole("alert")).toHaveTextContent("Something went wrong.");
    expect(within(card()).queryByRole("group", { name: "Preferred installation" })).not.toBeInTheDocument();
  });
});

// These check the markup and the text a screen reader is given. They cannot show what a screen
// reader actually says; that needs a manual pass with Narrator (or another screen reader).
describe("RobloxStatusCard: spoken status", () => {
  afterEach(() => vi.restoreAllMocks());

  const spoken = () => within(card()).getByRole("status");

  it("is one always-mounted, screen-reader-only, atomic, polite status", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED) });
    render(<RobloxStatusCard />);
    const status = spoken(); // present while still loading
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(status).toHaveAttribute("aria-atomic", "true");
    expect(status).toHaveClass("sr-only");

    await within(card()).findByText("Roblox files found");
    expect(spoken()).toBe(status); // the same element after the result arrives
  });

  it("says a whole sentence while loading, when offline, and when a check has finished", async () => {
    // Offline.
    const offline = render(<RobloxStatusCard />);
    await within(card()).findByText(/only works inside the Nexora window/);
    expect(spoken()).toHaveTextContent(/^Roblox cannot be checked outside the Nexora window\.$/);
    offline.unmount();

    // Loading, then finished.
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED) });
    render(<RobloxStatusCard />);
    expect(spoken()).toHaveTextContent(/^Looking for Roblox\.$/);
    await within(card()).findByText("Roblox files found");
    expect(spoken()).toHaveTextContent(/^Roblox check finished at .+: Roblox files found\.$/);
  });

  it("names the result, so it does not depend on the time alone", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_NOT_DETECTED) });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Not detected");
    expect(spoken()).toHaveTextContent(/: Not detected\.$/);
  });

  it("changes during a check, so a check ending in the same second is still announced", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000); // every check gets the same time
    let finish: (value: unknown) => void = () => {};
    let calls = 0;
    mockTauri({
      get_install_selection: () => {
        calls += 1;
        return calls === 1 ? selection(ROBLOX_DETECTED) : new Promise((resolve) => (finish = resolve));
      },
    });
    render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    const before = spoken().textContent;
    expect(before).toMatch(/^Roblox check finished at .+: Roblox files found\.$/);

    fireEvent.click(within(card()).getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(spoken()).toHaveTextContent(/^Checking for Roblox again\.$/));

    finish(selection(ROBLOX_DETECTED));
    await waitFor(() => expect(spoken()).toHaveTextContent(/^Roblox check finished at /));
    // Same words as before, but the text in between was different, so the change was announced.
    expect(spoken().textContent).toBe(before);
  });

  it("leaves a failed read to its own alert, and says nothing else", async () => {
    mockTauri({
      get_install_selection: () => {
        throw { code: "internal", message: "Something went wrong." };
      },
    });
    render(<RobloxStatusCard />);
    expect(await within(card()).findByRole("alert")).toHaveTextContent("Something went wrong.");
    expect(spoken()).toBeEmptyDOMElement();
  });

  it("does not make the visible result live as well, so nothing is announced twice", async () => {
    mockTauri({ get_install_selection: () => selection(ROBLOX_DETECTED, PREF_AUTOMATIC, BOTH_SOURCES) });
    const { container } = render(<RobloxStatusCard />);
    await within(card()).findByText("Roblox files found");
    const live = Array.from(container.querySelectorAll('[aria-live], [role="status"], [role="alert"]'));
    // The card's spoken status and the installation panel's own region: nothing else.
    expect(live).toHaveLength(2);
    expect(live[0]).toBe(spoken());
    expect(live[0]).not.toContainElement(within(card()).getByText("Roblox files found"));
    expect(within(card()).getByText("Roblox files found").closest("[aria-live]")).toBeNull();
  });
});
