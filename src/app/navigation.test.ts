import { describe, expect, it } from "vitest";
import { DEFAULT_ROUTE, hrefFor, NAV_ITEMS, routeFromHash } from "./navigation";

describe("NAV_ITEMS", () => {
  it("lists the eight sections in the specified order", () => {
    expect(NAV_ITEMS.map((i) => i.label)).toEqual([
      "Dashboard",
      "Accounts",
      "Game Launcher",
      "Automation",
      "Script Hub",
      "Performance",
      "Activity Logs",
      "Settings",
    ]);
  });

  it("has unique ids and paths, and every path starts with a slash", () => {
    expect(new Set(NAV_ITEMS.map((i) => i.id)).size).toBe(NAV_ITEMS.length);
    expect(new Set(NAV_ITEMS.map((i) => i.path)).size).toBe(NAV_ITEMS.length);
    expect(NAV_ITEMS.every((i) => i.path.startsWith("/"))).toBe(true);
  });

  it("builds hash hrefs", () => {
    const accounts = NAV_ITEMS.find((i) => i.id === "accounts");
    expect(accounts && hrefFor(accounts)).toBe("#/accounts");
  });
});

describe("routeFromHash", () => {
  it("finds each route from its own href", () => {
    for (const item of NAV_ITEMS) expect(routeFromHash(hrefFor(item)).id).toBe(item.id);
  });

  it("falls back to the dashboard for an empty, root or unknown hash", () => {
    for (const hash of ["", "#", "#/", "#/nope", "#/accounts/extra", "garbage"]) {
      expect(routeFromHash(hash).id).toBe(DEFAULT_ROUTE);
    }
  });

  it("ignores a query string or second fragment after the path", () => {
    expect(routeFromHash("#/logs?filter=error").id).toBe("logs");
    expect(routeFromHash("#/settings#top").id).toBe("settings");
  });
});
