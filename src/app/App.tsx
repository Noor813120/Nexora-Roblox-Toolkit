import { AppShell } from "../components/layout/AppShell";

/**
 * Application root. Navigation lives in the URL hash (`navigation.ts`), pages load lazily
 * (`pages.ts`), and each page is wrapped in its own error boundary (`AppShell`).
 */
export function App() {
  return <AppShell />;
}
