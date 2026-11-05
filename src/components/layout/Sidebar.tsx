import { memo } from "react";
import { hrefFor, NAV_ITEMS, type NavItem, type RouteId } from "../../app/navigation";

const NavLink = memo(function NavLink({ item, active }: { item: NavItem; active: boolean }) {
  const Icon = item.icon;
  return (
    <a
      href={hrefFor(item)}
      aria-current={active ? "page" : undefined}
      className={`ui-transition flex items-center gap-3 rounded-md px-3 py-2 text-sm ${
        active ? "bg-raised font-medium text-ink" : "text-muted hover:bg-raised hover:text-ink"
      }`}
    >
      <Icon aria-hidden="true" className={`h-4 w-4 shrink-0 ${active ? "text-accent" : ""}`} strokeWidth={1.75} />
      {item.label}
    </a>
  );
});

/** Left navigation. Re-renders only when the active route changes (items are static and memoized). */
export const Sidebar = memo(function Sidebar({ current }: { current: RouteId }) {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-line bg-surface">
      <div className="px-5 py-5">
        <p className="text-sm font-semibold tracking-wide">Nexora</p>
      </div>
      <nav aria-label="Main" className="flex-1 px-3">
        <ul className="space-y-0.5">
          {NAV_ITEMS.map((item) => (
            <li key={item.id}>
              <NavLink item={item} active={item.id === current} />
            </li>
          ))}
        </ul>
      </nav>
    </aside>
  );
});
