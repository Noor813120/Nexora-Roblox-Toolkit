import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend, makeLog, mockTauri } from "../../test/mockTauri";
import { LogsPage } from "./LogsPage";

const rows = () => within(screen.getByRole("table")).getAllByRole("row").slice(1);
const logCalls = (invoke: ReturnType<typeof mockTauri>) =>
  invoke.mock.calls.filter(([c]) => c === "list_activity_logs").map(([, a]) => (a as { query: Record<string, unknown> }).query);

describe("LogsPage states", () => {
  it("shows a loading message, then the entries with time, severity, event and message", async () => {
    mockTauri(
      backend({
        list_activity_logs: () => ({
          items: [
            makeLog(2, { severity: "error", eventType: "database.recovered", message: "Replaced a damaged file", metadataJson: '{"movedTo":"quarantine/x.db"}' }),
            makeLog(1, { message: "Nexora 0.1.0 started." }),
          ],
          nextBeforeId: null,
        }),
      }),
    );
    render(<LogsPage />);
    expect(screen.getByText("Loading activity…")).toBeInTheDocument();
    await screen.findByRole("table");
    const [newest, oldest] = rows() as [HTMLElement, HTMLElement];
    expect(within(newest).getByText("Error")).toBeInTheDocument();
    expect(within(newest).getByText("database.recovered")).toBeInTheDocument();
    expect(within(newest).getByText("Replaced a damaged file")).toBeInTheDocument();
    expect(within(newest).getByText("Details")).toBeInTheDocument();
    expect(within(newest).getByText(/quarantine\/x\.db/)).toBeInTheDocument();
    expect(within(oldest).getByText("Info")).toBeInTheDocument();
    expect(within(oldest).queryByText("Details")).toBeNull();
    expect(within(newest).getByText(/2025|2026/)).toBeInTheDocument();
    expect(screen.getByText("That is everything that is stored.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Load older entries" })).toBeNull();
  });

  it("shows an honest empty state, and no table, when nothing is recorded", async () => {
    mockTauri(backend());
    render(<LogsPage />);
    expect(await screen.findByText("No activity recorded")).toBeInTheDocument();
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.queryByText(/Planned for Phase/)).toBeNull();
  });

  it("shows the backend's error with a Try again that recovers", async () => {
    let fail = true;
    mockTauri(
      backend({
        list_activity_logs: () => {
          if (fail) throw { code: "unavailable", message: "the database is not available" };
          return { items: [makeLog(1, { message: "Recovered" })], nextBeforeId: null };
        },
      }),
    );
    render(<LogsPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent("the database is not available");
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Recovered")).toBeInTheDocument();
  });

  it("explains itself outside the app window and disables Refresh", async () => {
    render(<LogsPage />);
    expect(await screen.findByText(/only available inside the Nexora window/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Remove old entries…" })).toBeDisabled();
  });
});

describe("LogsPage filtering and paging", () => {
  it("filters by severity, asking the backend rather than hiding rows", async () => {
    const invoke = mockTauri(
      backend({
        list_activity_logs: (args) => {
          const q = (args as { query: { severity: string | null } }).query;
          return q.severity === "error"
            ? { items: [makeLog(9, { severity: "error", message: "only errors" })], nextBeforeId: null }
            : { items: [makeLog(1, { message: "everything" })], nextBeforeId: null };
        },
      }),
    );
    render(<LogsPage />);
    await screen.findByText("everything");
    fireEvent.change(screen.getByLabelText("Severity"), { target: { value: "error" } });
    expect(await screen.findByText("only errors")).toBeInTheDocument();
    expect(screen.queryByText("everything")).toBeNull();
    expect(logCalls(invoke).map((q) => q.severity)).toEqual([null, "error"]);
    fireEvent.change(screen.getByLabelText("Severity"), { target: { value: "" } });
    expect(await screen.findByText("everything")).toBeInTheDocument();
  });

  it("says so when a filter matches nothing", async () => {
    mockTauri(backend());
    render(<LogsPage />);
    await screen.findByText("No activity recorded");
    fireEvent.change(screen.getByLabelText("Severity"), { target: { value: "warning" } });
    expect(await screen.findByText("No warning entries")).toBeInTheDocument();
  });

  it("loads older entries on request and never shows an entry twice", async () => {
    const invoke = mockTauri(
      backend({
        list_activity_logs: (args) => {
          const q = (args as { query: { beforeId: number | null } }).query;
          return q.beforeId === null
            ? { items: [makeLog(4), makeLog(3)], nextBeforeId: 3 }
            : { items: [makeLog(3), makeLog(2), makeLog(1)], nextBeforeId: null }; // overlaps on purpose
        },
      }),
    );
    render(<LogsPage />);
    await screen.findByText("Entry 4");
    expect(rows()).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "Load older entries" }));
    await screen.findByText("Entry 1");
    expect(rows().map((r) => within(r).getByText(/^Entry \d$/).textContent)).toEqual(["Entry 4", "Entry 3", "Entry 2", "Entry 1"]);
    expect(screen.queryByRole("button", { name: "Load older entries" })).toBeNull();
    expect(logCalls(invoke).map((q) => [q.limit, q.beforeId])).toEqual([[50, null], [50, 3]]);
  });

  it("keeps what is shown when loading older entries fails, and says why", async () => {
    mockTauri(
      backend({
        list_activity_logs: (args) => {
          if ((args as { query: { beforeId: number | null } }).query.beforeId !== null) {
            throw { code: "internal", message: "internal error" };
          }
          return { items: [makeLog(2), makeLog(1)], nextBeforeId: 1 };
        },
      }),
    );
    render(<LogsPage />);
    await screen.findByText("Entry 2");
    fireEvent.click(screen.getByRole("button", { name: "Load older entries" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("internal error");
    expect(rows()).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Load older entries" })).toBeEnabled();
  });

  it("Refresh re-reads in place without blanking the list", async () => {
    let n = 0;
    const invoke = mockTauri(
      backend({ list_activity_logs: () => ({ items: [makeLog(++n, { message: `read ${n}` })], nextBeforeId: null }) }),
    );
    render(<LogsPage />);
    await screen.findByText("read 1");
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(screen.getByText("read 1")).toBeInTheDocument(); // still visible while refreshing
    await screen.findByText("read 2");
    expect(logCalls(invoke)).toHaveLength(2);
  });

  it("drops a slow reply that belongs to an old filter", async () => {
    let releaseSlow!: () => void;
    const slow = new Promise<void>((r) => (releaseSlow = r));
    mockTauri(
      backend({
        list_activity_logs: async (args) => {
          const sev = (args as { query: { severity: string | null } }).query.severity;
          if (sev === null) {
            await slow;
            return { items: [makeLog(1, { message: "stale unfiltered" })], nextBeforeId: null };
          }
          return { items: [makeLog(2, { message: "fresh filtered", severity: "error" })], nextBeforeId: null };
        },
      }),
    );
    render(<LogsPage />);
    fireEvent.change(screen.getByLabelText("Severity"), { target: { value: "error" } });
    await screen.findByText("fresh filtered");
    releaseSlow();
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByText("stale unfiltered")).toBeNull();
    expect(screen.getByText("fresh filtered")).toBeInTheDocument();
  });
});

describe("LogsPage clean-up", () => {
  const open = async () => {
    await screen.findByText("No activity recorded");
    fireEvent.click(screen.getByRole("button", { name: "Remove old entries…" }));
  };

  it("needs a confirming click, then removes and refreshes the list", async () => {
    const invoke = mockTauri(backend({ clear_old_activity_logs: () => 12 }));
    render(<LogsPage />);
    await open();
    expect(invoke.mock.calls.some(([c]) => c === "clear_old_activity_logs")).toBe(false);
    fireEvent.change(screen.getByLabelText("Remove entries older than (days)"), { target: { value: "14" } });
    fireEvent.click(screen.getByRole("button", { name: "Remove entries older than 14 days" }));
    expect(await screen.findByText("Removed 12 entries.")).toBeInTheDocument();
    const call = invoke.mock.calls.find(([c]) => c === "clear_old_activity_logs");
    expect(call?.[1]).toEqual({ olderThanDays: 14 });
    await waitFor(() => expect(logCalls(invoke)).toHaveLength(2)); // list reloaded
  });

  it("reports when nothing was old enough", async () => {
    mockTauri(backend({ clear_old_activity_logs: () => 0 }));
    render(<LogsPage />);
    await open();
    fireEvent.click(screen.getByRole("button", { name: /Remove entries older than 30 days/ }));
    expect(await screen.findByText("Nothing was old enough to remove.")).toBeInTheDocument();
  });

  it("refuses an invalid number of days without calling the backend", async () => {
    const invoke = mockTauri(backend());
    render(<LogsPage />);
    await open();
    const input = screen.getByLabelText("Remove entries older than (days)");
    for (const bad of ["0", "-3", "5000", "1.5", ""]) {
      fireEvent.change(input, { target: { value: bad } });
      expect(screen.getByRole("button", { name: /^Remove entries older than/ })).toBeDisabled();
    }
    expect(screen.getByText("Enter a whole number from 1 to 3650.")).toBeInTheDocument();
    expect(invoke.mock.calls.some(([c]) => c === "clear_old_activity_logs")).toBe(false);
  });

  it("can be cancelled, and shows a backend error", async () => {
    mockTauri(
      backend({
        clear_old_activity_logs: () => {
          throw { code: "internal", message: "internal error" };
        },
      }),
    );
    render(<LogsPage />);
    await open();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("button", { name: "Remove old entries…" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Remove old entries…" }));
    fireEvent.click(screen.getByRole("button", { name: /Remove entries older than 30 days/ }));
    expect(await screen.findByText("internal error")).toBeInTheDocument();
  });
});
