import { Suspense, useEffect, useRef } from "react";
import { PAGES } from "../../app/pages";
import { useRoute } from "../../app/useRoute";
import { ErrorBoundary } from "../ui/ErrorBoundary";
import { PageSkeleton } from "../ui/PageSkeleton";
import { Sidebar } from "./Sidebar";

/**
 * The window frame: sidebar on the left, the current page on the right.
 *
 * On every navigation it updates the window title and moves keyboard focus to the page, so
 * screen-reader and keyboard users land at the top of the new content instead of on the link they
 * just activated.
 */
export function AppShell() {
  const route = useRoute();
  const mainRef = useRef<HTMLElement>(null);
  const firstRender = useRef(true);

  useEffect(() => {
    document.title = `${route.label} · RoALT Advanced`;
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    mainRef.current?.focus();
  }, [route.id, route.label]);

  const Page = PAGES[route.id];

  return (
    <div className="flex h-full">
      <button
        type="button"
        onClick={() => mainRef.current?.focus()}
        className="sr-only z-10 rounded-md bg-accent text-sm font-medium text-accent-ink focus:not-sr-only focus:absolute focus:left-3 focus:top-3 focus:px-3 focus:py-2"
      >
        Skip to content
      </button>
      <Sidebar current={route.id} />
      <main ref={mainRef} tabIndex={-1} className="min-w-0 flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-6xl">
          {/* Keyed by route so an error on one page is cleared by opening another. */}
          <ErrorBoundary key={route.id}>
            <Suspense fallback={<PageSkeleton />}>
              <Page />
            </Suspense>
          </ErrorBoundary>
        </div>
      </main>
    </div>
  );
}
