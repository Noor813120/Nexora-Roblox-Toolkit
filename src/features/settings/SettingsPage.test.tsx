import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend, DB_STATUS, mockTauri, SETTINGS } from "../../test/mockTauri";
import { SettingsPage } from "./SettingsPage";

const card = (name: string) => screen.getByRole("region", { name });
const count = (invoke: ReturnType<typeof mockTauri>, cmd: string) => invoke.mock.calls.filter(([c]) => c === cmd).length;

describe("Preferences (persisted settings)", () => {
  it("shows the stored values with their allowed range and default", async () => {
    mockTauri(backend());
    render(<SettingsPage />);
    const prefs = card("Preferences");
    const days = await within(prefs).findByLabelText("Keep activity history for");
    expect(days).toHaveValue(30);
    expect(within(prefs).getByLabelText("Entries on the Dashboard")).toHaveValue(8);
    expect(within(prefs).getAllByText(/Allowed: 1 to 365\. Default: 30\./)).toHaveLength(1);
    // Nothing to save or reset yet.
    for (const b of within(prefs).getAllByRole("button", { name: /^Save/ })) expect(b).toBeDisabled();
    expect(within(prefs).queryByRole("button", { name: /Reset/ })).toBeNull();
    expect(screen.queryByText(/Planned for Phase/)).toBeNull();
  });

  it("saves a changed value through set_setting and reads the stored result back", async () => {
    let stored = "30";
    const invoke = mockTauri(
      backend({
        list_settings: () => [{ ...SETTINGS[0]!, value: stored, isDefault: stored === "30" }, SETTINGS[1]!],
        set_setting: (a) => {
          stored = (a as { value: string }).value;
          return {};
        },
      }),
    );
    render(<SettingsPage />);
    const input = await within(card("Preferences")).findByLabelText("Keep activity history for");
    const save = screen.getByRole("button", { name: "Save Keep activity history for" });
    fireEvent.change(input, { target: { value: "90" } });
    expect(save).toBeEnabled();
    fireEvent.click(save);
    expect(await screen.findByText("Saved.")).toBeInTheDocument();
    const call = invoke.mock.calls.find(([c]) => c === "set_setting");
    expect(call?.[1]).toEqual({ key: "activity.retention_days", value: "90" });
    // The value shown is the one the backend now reports: it is no longer the default, so a reset
    // is offered. (Waiting on the reset button, not on the input, because the input already shows
    // what was typed before the reload finishes.)
    expect(await screen.findByRole("button", { name: "Reset Keep activity history for to 30" })).toBeInTheDocument();
    expect(screen.getByLabelText("Keep activity history for")).toHaveValue(90);
    expect(screen.getByRole("button", { name: "Save Keep activity history for" })).toBeDisabled(); // nothing unsaved
    expect(count(invoke, "list_settings")).toBe(2);
  });

  it("rejects out-of-range or non-integer input before calling the backend", async () => {
    const invoke = mockTauri(backend());
    render(<SettingsPage />);
    const input = await within(card("Preferences")).findByLabelText("Keep activity history for");
    const save = screen.getByRole("button", { name: "Save Keep activity history for" });
    for (const bad of ["0", "366", "-1", "2.5", "", "abc"]) {
      fireEvent.change(input, { target: { value: bad } });
      expect(save).toBeDisabled();
    }
    fireEvent.change(input, { target: { value: "0" } });
    expect(screen.getByText("Enter a whole number from 1 to 365.")).toBeInTheDocument();
    expect(count(invoke, "set_setting")).toBe(0);
  });

  it("shows the backend's refusal and keeps the typed value", async () => {
    mockTauri(
      backend({
        set_setting: () => {
          throw { code: "invalid", message: "invalid request: activity.retention_days must be between 1 and 365" };
        },
      }),
    );
    render(<SettingsPage />);
    const input = await within(card("Preferences")).findByLabelText("Keep activity history for");
    fireEvent.change(input, { target: { value: "60" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Keep activity history for" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("must be between 1 and 365");
    expect(input).toHaveValue(60);
  });

  it("resets a saved value to its default through delete_setting", async () => {
    let saved = true;
    const invoke = mockTauri(
      backend({
        list_settings: () => [
          saved ? { ...SETTINGS[0]!, value: "90", isDefault: false, updatedAt: 5 } : SETTINGS[0]!,
          SETTINGS[1]!,
        ],
        delete_setting: () => {
          saved = false;
          return SETTINGS[0];
        },
      }),
    );
    render(<SettingsPage />);
    fireEvent.click(await screen.findByRole("button", { name: "Reset Keep activity history for to 30" }));
    await waitFor(() => expect(screen.getByLabelText("Keep activity history for")).toHaveValue(30));
    expect(invoke.mock.calls.find(([c]) => c === "delete_setting")?.[1]).toEqual({ key: "activity.retention_days" });
    expect(screen.queryByRole("button", { name: /Reset Keep activity/ })).toBeNull();
  });

  it("explains when the database is unavailable instead of showing fake values", async () => {
    mockTauri(
      backend({
        list_settings: () => {
          throw { code: "unavailable", message: "the database is not available" };
        },
      }),
    );
    render(<SettingsPage />);
    expect(await within(card("Preferences")).findByText("the database is not available")).toBeInTheDocument();
    expect(within(card("Preferences")).queryByRole("spinbutton")).toBeNull();
  });

  it("is honest outside the app window", async () => {
    render(<SettingsPage />);
    expect(await within(card("Preferences")).findByText(/only available inside the Nexora window/)).toBeInTheDocument();
    expect(await within(card("Database")).findByText(/only available inside/)).toBeInTheDocument();
  });
});

describe("Database card", () => {
  it("shows where the data is and how it is stored", async () => {
    mockTauri(backend());
    render(<SettingsPage />);
    const db = card("Database");
    expect(await within(db).findByText("Working")).toBeInTheDocument();
    expect(within(db).getByText(DB_STATUS.location)).toBeInTheDocument();
    expect(within(db).getByText("wal")).toBeInTheDocument();
    expect(within(db).getByText("52.0 KB")).toBeInTheDocument();
  });

  it("when the database is unavailable: says why and disables backup and restore", async () => {
    mockTauri(
      backend({
        database_status: () => ({
          ...DB_STATUS,
          available: false,
          schemaVersion: null,
          journalMode: null,
          sizeBytes: null,
          error: "The app does not have permission to use its data folder.",
        }),
      }),
    );
    render(<SettingsPage />);
    const db = card("Database");
    expect(await within(db).findByText("Unavailable")).toBeInTheDocument();
    expect(within(db).getByRole("alert")).toHaveTextContent("does not have permission");
    expect(within(db).getByRole("button", { name: "Create backup…" })).toBeDisabled();
    expect(within(db).getByRole("button", { name: "Restore from backup…" })).toBeDisabled();
  });

  it("creates a backup and reports the file name and size", async () => {
    const invoke = mockTauri(backend({ backup_database: () => ({ saved: true, fileName: "my-backup.db", sizeBytes: 40960 }) }));
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    fireEvent.click(screen.getByRole("button", { name: "Create backup…" }));
    expect(await screen.findByText("Backup saved as my-backup.db (40.0 KB).")).toBeInTheDocument();
    expect(count(invoke, "backup_database")).toBe(1);
  });

  it("says nothing was saved when the dialog is closed", async () => {
    mockTauri(backend({ backup_database: () => ({ saved: false, fileName: null, sizeBytes: null }) }));
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    fireEvent.click(screen.getByRole("button", { name: "Create backup…" }));
    expect(await screen.findByText("No file was chosen. Nothing was saved.")).toBeInTheDocument();
  });

  it("shows a clear error when the backup is refused", async () => {
    mockTauri(
      backend({
        backup_database: () => {
          throw { code: "invalid", message: "invalid request: a file with that name already exists; it was not overwritten" };
        },
      }),
    );
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    fireEvent.click(screen.getByRole("button", { name: "Create backup…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("was not overwritten");
  });

  it("never sends two backups for a double click", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const invoke = mockTauri(
      backend({
        backup_database: async () => {
          await gate;
          return { saved: true, fileName: "a.db", sizeBytes: 10 };
        },
      }),
    );
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    const button = screen.getByRole("button", { name: "Create backup…" });
    fireEvent.click(button);
    fireEvent.click(button);
    release();
    await screen.findByText(/Backup saved as a\.db/);
    expect(count(invoke, "backup_database")).toBe(1);
  });

  it("asks for confirmation before restoring, and Cancel changes nothing", async () => {
    const invoke = mockTauri(backend());
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    fireEvent.click(screen.getByRole("button", { name: "Restore from backup…" }));
    const dialog = screen.getByRole("alertdialog", { name: "Restore from a backup?" });
    expect(dialog).toHaveTextContent(/replaced by the contents of the backup file/);
    expect(count(invoke, "restore_database")).toBe(0);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(count(invoke, "restore_database")).toBe(0);
  });

  it("restores after confirmation, reports the safety copy, and re-reads the preferences", async () => {
    const invoke = mockTauri(
      backend({
        restore_database: () => ({ restored: true, schemaVersion: 1, migratedVersions: [], safetyCopy: "pre-restore-20261104-120000.db" }),
      }),
    );
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    expect(count(invoke, "list_settings")).toBe(1);
    fireEvent.click(screen.getByRole("button", { name: "Restore from backup…" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose a backup file…" }));
    expect(await screen.findByText(/Restored\. Your previous data was kept as pre-restore-20261104-120000\.db/)).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await waitFor(() => expect(count(invoke, "list_settings")).toBe(2));
    expect(count(invoke, "database_status")).toBe(2);
  });

  it("does not reload anything when the file dialog is closed without a choice", async () => {
    const invoke = mockTauri(
      backend({ restore_database: () => ({ restored: false, schemaVersion: null, migratedVersions: [], safetyCopy: null }) }),
    );
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    fireEvent.click(screen.getByRole("button", { name: "Restore from backup…" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose a backup file…" }));
    expect(await screen.findByText("No file was chosen. Nothing was changed.")).toBeInTheDocument();
    expect(count(invoke, "list_settings")).toBe(1);
  });

  it("shows why a bad backup was refused, and mentions an upgrade when one happened", async () => {
    let refuse = true;
    mockTauri(
      backend({
        restore_database: () => {
          if (refuse) throw { code: "invalid", message: "invalid request: the backup was made by a newer version of Nexora" };
          return { restored: true, schemaVersion: 2, migratedVersions: [2], safetyCopy: "pre-restore-x.db" };
        },
      }),
    );
    render(<SettingsPage />);
    await within(card("Database")).findByText("Working");
    fireEvent.click(screen.getByRole("button", { name: "Restore from backup…" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose a backup file…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("newer version");
    refuse = false;
    fireEvent.click(screen.getByRole("button", { name: "Restore from backup…" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose a backup file…" }));
    expect(await screen.findByText(/upgraded to the current format/)).toBeInTheDocument();
  });
});
