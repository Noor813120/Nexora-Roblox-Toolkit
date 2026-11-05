import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { NAV_ITEMS } from "../../app/navigation";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("is a labelled navigation landmark with one link per section, in order", () => {
    render(<Sidebar current="dashboard" />);
    const nav = screen.getByRole("navigation", { name: "Main" });
    const links = within(nav).getAllByRole("link");
    expect(links.map((a) => a.textContent)).toEqual(NAV_ITEMS.map((i) => i.label));
    expect(links.map((a) => a.getAttribute("href"))).toEqual(NAV_ITEMS.map((i) => `#${i.path}`));
  });

  it("marks only the current page with aria-current", () => {
    render(<Sidebar current="scripts" />);
    const current = screen.getAllByRole("link").filter((a) => a.getAttribute("aria-current") === "page");
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveTextContent("Script Hub");
  });

  it("hides decorative icons from assistive technology", () => {
    const { container } = render(<Sidebar current="dashboard" />);
    const icons = container.querySelectorAll("nav svg");
    expect(icons).toHaveLength(NAV_ITEMS.length);
    icons.forEach((svg) => expect(svg).toHaveAttribute("aria-hidden", "true"));
  });
});
