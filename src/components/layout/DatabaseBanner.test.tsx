import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { backend, DB_STATUS, mockTauri } from "../../test/mockTauri";
import { DatabaseBanner } from "./DatabaseBanner";

describe("DatabaseBanner", () => {
  it("renders nothing when the database is healthy", async () => {
    const invoke = mockTauri(backend());
    const { container } = render(<DatabaseBanner />);
    await new Promise((r) => setTimeout(r, 10));
    expect(invoke.mock.calls.map(([c]) => c)).toEqual(["database_status"]);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing outside the app window, while loading, or if the status call fails", async () => {
    const { container, unmount } = render(<DatabaseBanner />);
    await new Promise((r) => setTimeout(r, 10));
    expect(container).toBeEmptyDOMElement();
    unmount();
    mockTauri(
      backend({
        database_status: () => {
          throw { code: "internal", message: "internal error" };
        },
      }),
    );
    const second = render(<DatabaseBanner />);
    await new Promise((r) => setTimeout(r, 10));
    expect(second.container).toBeEmptyDOMElement();
  });

  it("warns loudly, with the reason, when the database could not be opened", async () => {
    mockTauri(
      backend({
        database_status: () => ({
          ...DB_STATUS,
          available: false,
          error: "This database was created by a newer version of RoALT Advanced. Update the app to use it. Nothing was changed.",
        }),
      }),
    );
    render(<DatabaseBanner />);
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Saved data is unavailable");
    expect(alert).toHaveTextContent("newer version of RoALT Advanced");
  });

  it("tells the user a damaged database was replaced, names the preserved file, and can be dismissed", async () => {
    mockTauri(backend({ database_status: () => ({ ...DB_STATUS, recoveredFile: "roalt-20261104-120000.db" }) }));
    render(<DatabaseBanner />);
    const note = await screen.findByRole("status");
    expect(note).toHaveTextContent("damaged and has been replaced");
    expect(note).toHaveTextContent("roalt-20261104-120000.db");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
