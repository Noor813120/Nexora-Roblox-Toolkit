import {
  FileCode,
  Gauge,
  LayoutDashboard,
  Rocket,
  ScrollText,
  Settings,
  Users,
  Workflow,
  type LucideIcon,
} from "lucide-react";

export type RouteId =
  | "dashboard"
  | "accounts"
  | "launcher"
  | "automation"
  | "scripts"
  | "performance"
  | "logs"
  | "settings";

export interface NavItem {
  id: RouteId;
  /** URL hash path, e.g. `#/accounts`. Hash routing needs no server and works in the Tauri webview. */
  path: string;
  label: string;
  /** One line shown on the Dashboard's quick navigation. */
  description: string;
  icon: LucideIcon;
}

/** The sidebar, in display order. The order is part of the product spec; tests pin it. */
export const NAV_ITEMS: readonly NavItem[] = [
  { id: "dashboard", path: "/dashboard", label: "Dashboard", description: "Status and system overview", icon: LayoutDashboard },
  { id: "accounts", path: "/accounts", label: "Accounts", description: "Account profiles and launch settings", icon: Users },
  { id: "launcher", path: "/launcher", label: "Game Launcher", description: "Game targets, queue and running clients", icon: Rocket },
  { id: "automation", path: "/automation", label: "Automation", description: "Auto-join, reconnect and idle alerts", icon: Workflow },
  { id: "scripts", path: "/scripts", label: "Script Hub", description: "Automation scripts and Luau for Studio", icon: FileCode },
  { id: "performance", path: "/performance", label: "Performance", description: "CPU, memory and resource limits", icon: Gauge },
  { id: "logs", path: "/logs", label: "Activity Logs", description: "What the app and its tasks did", icon: ScrollText },
  { id: "settings", path: "/settings", label: "Settings", description: "Preferences and about", icon: Settings },
];

export const DEFAULT_ROUTE: RouteId = "dashboard";

/** The `href` for a nav item. */
export const hrefFor = (item: NavItem): string => `#${item.path}`;

/**
 * Maps a `location.hash` value to a route. An empty or unknown hash falls back to the Dashboard,
 * so a stale or hand-edited URL never leaves a blank screen.
 */
export function routeFromHash(hash: string): NavItem {
  const path = hash.replace(/^#/, "").split(/[?#]/, 1)[0] ?? "";
  const found = NAV_ITEMS.find((item) => item.path === path);
  // NAV_ITEMS is a non-empty constant, so the fallback lookup cannot fail.
  return found ?? (NAV_ITEMS.find((item) => item.id === DEFAULT_ROUTE) as NavItem);
}
