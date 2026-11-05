import { lazy, type ComponentType, type LazyExoticComponent } from "react";
import type { RouteId } from "./navigation";

/**
 * Every page is its own chunk, downloaded the first time it is opened. The Dashboard is the
 * landing page, so it is the only one most sessions need up front.
 */
export const PAGES: Record<RouteId, LazyExoticComponent<ComponentType>> = {
  dashboard: lazy(() => import("../features/dashboard/DashboardPage").then((m) => ({ default: m.DashboardPage }))),
  accounts: lazy(() => import("../features/accounts/AccountsPage").then((m) => ({ default: m.AccountsPage }))),
  launcher: lazy(() => import("../features/launcher/LauncherPage").then((m) => ({ default: m.LauncherPage }))),
  automation: lazy(() => import("../features/automation/AutomationPage").then((m) => ({ default: m.AutomationPage }))),
  scripts: lazy(() => import("../features/scripts/ScriptsPage").then((m) => ({ default: m.ScriptsPage }))),
  performance: lazy(() => import("../features/performance/PerformancePage").then((m) => ({ default: m.PerformancePage }))),
  logs: lazy(() => import("../features/logs/LogsPage").then((m) => ({ default: m.LogsPage }))),
  settings: lazy(() => import("../features/settings/SettingsPage").then((m) => ({ default: m.SettingsPage }))),
};
