import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { InstallSelection, PreferredSource } from "../../api";
import {
  BOTH_SOURCES,
  INSTALL_FOLDER,
  INSTALL_HANDLER,
  mockTauri,
  PREF_AUTOMATIC,
  PREF_AUTOMATIC_NONE,
  PREF_AVAILABLE_FOLDERS,
  PREF_AVAILABLE_HANDLER,
  PREF_INCONCLUSIVE,
  PREF_INVALID,
  PREF_NEWER,
  PREF_UNAVAILABLE,
  PREF_UNSUPPORTED_PLATFORM,
  PREF_UNSUPPORTED_SOURCE,
  ROBLOX_DETECTED,
  selection,
} from "../../test/mockTauri";
import { InstallPreferencePanel } from "./InstallPreferencePanel";

type Pref = Parameters<typeof selection>[1];

/**
 * The sources the backend would list alongside a saved preference, by its own contract: a source
 * is listed only if resolving it gives "available". So a saved source that is not available, or
 * could not be checked, is never in its own list, and on an unsupported system the list is empty.
 */
const consistentSources = (preference: Pref): PreferredSource[] => {
  if (
    preference &&
    (preference.state === "unavailable" || preference.state === "inconclusive") &&
    preference.preferred
  ) {
    return BOTH_SOURCES.filter((source) => source !== preference.preferred);
  }
  if (preference?.state === "unsupported" && preference.reason === "unsupportedPlatform") return [];
  return BOTH_SOURCES;
};

/** A selection as the backend would send it; the panel only needs the preference and the sources. */
const sel = (preference: Pref, sources = consistentSources(preference)) =>
  selection(ROBLOX_DETECTED, preference, sources) as unknown as InstallSelection;

const radio = (name: RegExp | string) => screen.getByRole("radio", { name }) as HTMLInputElement;
const saveButton = () => screen.getByRole("button", { name: /^(Save choice|Saving…)$/ }) as HTMLButtonElement;
const group = () => screen.getByRole("group", { name: "Preferred installation" });
/** Every `set_preferred_install` call as [command, arguments] (invoke also records an options slot). */
const setCalls = (invoke: ReturnType<typeof mockTauri>) =>
  invoke.mock.calls.filter(([cmd]) => cmd === "set_preferred_install").map(([cmd, args]) => [cmd, args]);

describe("InstallPreferencePanel: what the saved choice means", () => {
  const cases: Array<[string, Pref, string, RegExp | string]> = [
    ["automatic, with an install", PREF_AUTOMATIC, "Automatic", /Using the first installation found/],
    ["automatic, nothing to use", PREF_AUTOMATIC_NONE, "Automatic", /No installation was found to use/],
    ["available: version folders", PREF_AVAILABLE_FOLDERS, "Available", /Roblox version folders: using 1 of 2 installations found/],
    ["available: link handler", PREF_AVAILABLE_HANDLER, "Available", /Windows link handler: using 1 of 1 installation found/],
    ["not available", PREF_UNAVAILABLE, "Not available", /Your saved choice, Windows link handler, was not found this time\. Nothing else was used in its place/],
    [
      "could not tell",
      PREF_INCONCLUSIVE,
      "Could not tell",
      "Some places could not be checked, so the saved Roblox version folders may still exist. Nothing else was used in its place.",
    ],
    ["unreadable", PREF_INVALID, "Unreadable", /could not be read, so it was left unchanged/],
    ["newer version", PREF_NEWER, "Not supported here", /written by a newer version of Nexora/],
    ["source not supported", PREF_UNSUPPORTED_SOURCE, "Not supported here", /names a source this version cannot use/],
    ["not Windows", PREF_UNSUPPORTED_PLATFORM, "Not supported here", /can only be used on Windows/],
  ];

  it.each(cases)("says so plainly: %s", (_name, preference, pill, text) => {
    mockTauri({});
    const { container } = render(<InstallPreferencePanel selection={sel(preference)} onSaved={vi.fn()} />);
    // Looked up inside the status region: "Automatic" is also the name of an option below it.
    const status = within(container.querySelector('[aria-live="polite"]') as HTMLElement);
    expect(status.getByText(pill)).toBeInTheDocument();
    expect(status.getByText(text)).toBeInTheDocument();
  });

  it("names the saved source with its capital letters, whichever source it is", () => {
    mockTauri({});
    const handler = { ...PREF_INCONCLUSIVE, preferred: "protocolHandler" as const };
    const { container } = render(<InstallPreferencePanel selection={sel(handler)} onSaved={vi.fn()} />);
    const status = within(container.querySelector('[aria-live="polite"]') as HTMLElement);
    expect(
      status.getByText(
        "Some places could not be checked, so the saved Windows link handler may still exist. Nothing else was used in its place.",
      ),
    ).toBeInTheDocument();
  });

  it("shows which program would be used, for an available or automatic choice only", () => {
    mockTauri({});
    const { rerender } = render(<InstallPreferencePanel selection={sel(PREF_AVAILABLE_FOLDERS)} onSaved={vi.fn()} />);
    expect(screen.getByText(`Program: ${INSTALL_FOLDER.executablePath}`)).toBeInTheDocument();
    rerender(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={vi.fn()} />);
    expect(screen.getByText(`Program: ${INSTALL_HANDLER.executablePath}`)).toBeInTheDocument();
    for (const unusable of [PREF_UNAVAILABLE, PREF_INCONCLUSIVE, PREF_INVALID, PREF_NEWER]) {
      rerender(<InstallPreferencePanel selection={sel(unusable)} onSaved={vi.fn()} />);
      expect(screen.queryByText(/^Program:/)).not.toBeInTheDocument();
    }
  });

  it("an unreadable preference store is not 'Automatic', and nothing can be chosen or saved", () => {
    mockTauri({});
    const { container } = render(<InstallPreferencePanel selection={sel(null)} onSaved={vi.fn()} />);
    expect(screen.getByText("Preference unavailable")).toBeInTheDocument();
    expect(screen.getByText(/Preference storage is unavailable/)).toBeInTheDocument();
    // Nothing in the status region says "Automatic" (the option below it is outside that region).
    const status = within(container.querySelector('[aria-live="polite"]') as HTMLElement);
    expect(status.queryByText("Automatic")).not.toBeInTheDocument();
    expect(group()).toBeDisabled();
    for (const r of screen.getAllByRole("radio")) {
      expect(r).toBeDisabled();
      expect(r).not.toBeChecked(); // "Automatic" is not implied
    }
    expect(saveButton()).toBeDisabled();
  });
});

describe("InstallPreferencePanel: the choice control", () => {
  it("is a labelled group of three radios, and offers no launch control", () => {
    mockTauri({});
    render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={vi.fn()} />);
    expect(group()).toBeInTheDocument();
    expect(screen.getAllByRole("radio").map((r) => (r as HTMLInputElement).value)).toEqual([
      "automatic",
      "protocolHandler",
      "versionFolder",
    ]);
    expect(radio("Automatic")).toBeInTheDocument();
    expect(radio("Windows link handler")).toBeInTheDocument();
    expect(radio("Roblox version folders")).toBeInTheDocument();
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Save choice"]);
  });

  it("offers only the sources the backend lists, and 'Automatic' always", () => {
    mockTauri({});
    const { rerender } = render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC, ["versionFolder"])} onSaved={vi.fn()} />);
    expect(radio("Automatic")).toBeEnabled();
    expect(radio(/Windows link handler/)).toBeDisabled();
    expect(screen.getByText("Windows link handler — not available now")).toBeInTheDocument();
    expect(radio("Roblox version folders")).toBeEnabled();

    rerender(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC, [])} onSaved={vi.fn()} />);
    expect(radio("Automatic")).toBeEnabled();
    expect(radio(/Windows link handler/)).toBeDisabled();
    expect(radio(/Roblox version folders/)).toBeDisabled();

    rerender(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC, BOTH_SOURCES)} onSaved={vi.fn()} />);
    expect(radio("Windows link handler")).toBeEnabled();
    expect(radio("Roblox version folders")).toBeEnabled();
  });

  const initial: Array<[string, Pref, string | null]> = [
    ["automatic", PREF_AUTOMATIC, "Automatic"],
    ["available source", PREF_AVAILABLE_FOLDERS, "Roblox version folders"],
    ["saved but not available: stays selected, not replaced", PREF_UNAVAILABLE, "Windows link handler — not available now"],
    ["saved, could not tell: stays selected, not offered", PREF_INCONCLUSIVE, "Roblox version folders — not available now"],
    ["saved, not Windows", PREF_UNSUPPORTED_PLATFORM, "Roblox version folders — not available now"],
    ["unreadable: nothing selected", PREF_INVALID, null],
    ["newer version: nothing selected", PREF_NEWER, null],
    ["source not supported: nothing selected", PREF_UNSUPPORTED_SOURCE, null],
  ];

  it.each(initial)("starts with the right option selected: %s", (_name, preference, selected) => {
    mockTauri({});
    // The sources offered come from `sel`, which follows the backend's contract for each state.
    render(<InstallPreferencePanel selection={sel(preference)} onSaved={vi.fn()} />);
    const checked = screen.getAllByRole("radio").filter((r) => (r as HTMLInputElement).checked);
    expect(checked.map((r) => r.parentElement?.querySelector("label")?.textContent ?? "")).toEqual(
      selected === null ? [] : [selected],
    );
  });

  it("saving is explicit: nothing to save until the choice differs from the saved one", () => {
    mockTauri({});
    render(<InstallPreferencePanel selection={sel(PREF_AVAILABLE_FOLDERS)} onSaved={vi.fn()} />);
    expect(saveButton()).toBeDisabled();
    fireEvent.click(radio("Windows link handler"));
    expect(saveButton()).toBeEnabled();
    fireEvent.click(radio("Roblox version folders")); // back to what is saved
    expect(saveButton()).toBeDisabled();
  });

  it("'Automatic' can replace a preference that cannot be used", () => {
    mockTauri({});
    for (const unusable of [PREF_INVALID, PREF_NEWER, PREF_UNSUPPORTED_SOURCE]) {
      const { unmount } = render(<InstallPreferencePanel selection={sel(unusable)} onSaved={vi.fn()} />);
      expect(saveButton()).toBeDisabled(); // nothing chosen yet
      fireEvent.click(radio("Automatic"));
      expect(saveButton()).toBeEnabled();
      unmount();
    }
  });

  it("keeps an unavailable saved source selected and disabled", () => {
    mockTauri({});
    render(<InstallPreferencePanel selection={sel(PREF_UNAVAILABLE, [])} onSaved={vi.fn()} />);
    const saved = radio(/Windows link handler/);
    expect(saved).toBeChecked();
    expect(saved).toBeDisabled();
    expect(radio("Automatic")).not.toBeChecked();
    expect(saveButton()).toBeDisabled();
  });

  it.each([
    ["not available", PREF_UNAVAILABLE],
    ["could not tell", PREF_INCONCLUSIVE],
    ["not Windows", PREF_UNSUPPORTED_PLATFORM],
  ] as Array<[string, Pref]>)(
    "a saved source that cannot be used now can be replaced with Automatic: %s",
    async (_name, preference) => {
      const invoke = mockTauri({
        set_preferred_install: () => selection(ROBLOX_DETECTED, PREF_AUTOMATIC, BOTH_SOURCES),
      });
      const onSaved = vi.fn();
      render(<InstallPreferencePanel selection={sel(preference)} onSaved={onSaved} />);
      expect(saveButton()).toBeDisabled(); // nothing to save while the saved choice is untouched

      fireEvent.click(radio("Automatic")); // the way out
      expect(saveButton()).toBeEnabled();
      fireEvent.click(saveButton());

      await waitFor(() => expect(onSaved).toHaveBeenCalledTimes(1));
      expect(setCalls(invoke)).toEqual([["set_preferred_install", { choice: "automatic" }]]);
    },
  );
});

describe("InstallPreferencePanel: saving", () => {
  it("sends exactly the chosen value, then asks for the whole selection again", async () => {
    const invoke = mockTauri({ set_preferred_install: () => selection(ROBLOX_DETECTED, PREF_AVAILABLE_FOLDERS, BOTH_SOURCES) });
    const onSaved = vi.fn();
    render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={onSaved} />);

    fireEvent.click(radio("Roblox version folders"));
    fireEvent.click(saveButton());
    await waitFor(() => expect(onSaved).toHaveBeenCalledTimes(1));

    expect(setCalls(invoke)).toEqual([["set_preferred_install", { choice: "versionFolder" }]]);
    expect(screen.getByRole("status")).toHaveTextContent("Saved.");
    expect(saveButton()).toBeDisabled(); // nothing more to save until something changes
  });

  it("sends 'automatic' for Automatic", async () => {
    const invoke = mockTauri({ set_preferred_install: () => selection(ROBLOX_DETECTED, PREF_AUTOMATIC, BOTH_SOURCES) });
    render(<InstallPreferencePanel selection={sel(PREF_AVAILABLE_HANDLER)} onSaved={vi.fn()} />);
    fireEvent.click(radio("Automatic"));
    fireEvent.click(saveButton());
    await waitFor(() => expect(setCalls(invoke)).toHaveLength(1));
    expect(setCalls(invoke)[0]).toEqual(["set_preferred_install", { choice: "automatic" }]);
  });

  it("shows 'Saving…', blocks the controls, and sends one request however often it is clicked", async () => {
    let finish: (value: unknown) => void = () => {};
    const invoke = mockTauri({
      set_preferred_install: () => new Promise((resolve) => (finish = resolve)),
    });
    const onSaved = vi.fn();
    render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={onSaved} />);
    fireEvent.click(radio("Windows link handler"));
    fireEvent.click(saveButton());

    const busy = await screen.findByRole("button", { name: "Saving…" });
    expect(busy).toBeDisabled();
    expect(group()).toBeDisabled();
    fireEvent.click(busy);
    fireEvent.click(busy);
    expect(setCalls(invoke)).toHaveLength(1);
    expect(onSaved).not.toHaveBeenCalled(); // not reloaded until the save has finished

    finish(selection(ROBLOX_DETECTED, PREF_AVAILABLE_HANDLER, BOTH_SOURCES));
    await waitFor(() => expect(onSaved).toHaveBeenCalledTimes(1));
    expect(group()).toBeEnabled();
  });

  it("a failed save shows the message, keeps the user's pick, and does not reload", async () => {
    const message = "That Roblox source is not available right now. Check again.";
    const invoke = mockTauri({
      set_preferred_install: () => {
        throw { code: "conflict", message };
      },
    });
    const onSaved = vi.fn();
    render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={onSaved} />);
    fireEvent.click(radio("Windows link handler"));
    fireEvent.click(saveButton());

    expect(await screen.findByRole("alert")).toHaveTextContent(message);
    expect(onSaved).not.toHaveBeenCalled();
    expect(radio("Windows link handler")).toBeChecked(); // the draft is kept
    expect(saveButton()).toBeEnabled(); // and can be retried
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(setCalls(invoke)).toHaveLength(1);

    // Picking something else clears the old message.
    fireEvent.click(radio("Roblox version folders"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("an unsaved pick survives a refresh that changes nothing, and follows a changed saved choice", () => {
    mockTauri({});
    const { rerender } = render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={vi.fn()} />);
    fireEvent.click(radio("Windows link handler"));
    rerender(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={vi.fn()} />); // "Check again", same answer
    expect(radio("Windows link handler")).toBeChecked();

    rerender(<InstallPreferencePanel selection={sel(PREF_AVAILABLE_FOLDERS)} onSaved={vi.fn()} />); // the saved choice changed
    expect(radio("Roblox version folders")).toBeChecked();
    expect(radio("Windows link handler")).not.toBeChecked();
  });

  it("never touches the backend until Save is pressed", () => {
    const invoke = mockTauri({});
    render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={vi.fn()} />);
    fireEvent.click(radio("Windows link handler"));
    fireEvent.click(radio("Automatic"));
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("InstallPreferencePanel: announcements", () => {
  it("keeps one live region mounted, so a changed status is announced", () => {
    mockTauri({});
    const { container, rerender } = render(<InstallPreferencePanel selection={sel(PREF_AUTOMATIC)} onSaved={vi.fn()} />);
    const region = container.querySelector('[aria-live="polite"]');
    expect(region).not.toBeNull();
    rerender(<InstallPreferencePanel selection={sel(PREF_UNAVAILABLE, [])} onSaved={vi.fn()} />);
    expect(container.querySelector('[aria-live="polite"]')).toBe(region); // the same element
    expect(region).toHaveTextContent("Not available");
    rerender(<InstallPreferencePanel selection={sel(null)} onSaved={vi.fn()} />);
    expect(container.querySelector('[aria-live="polite"]')).toBe(region);
    expect(region).toHaveTextContent("Preference storage is unavailable");
  });
});
