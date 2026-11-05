import { useSyncExternalStore } from "react";
import { routeFromHash, type NavItem } from "./navigation";

function subscribe(onChange: () => void): () => void {
  window.addEventListener("hashchange", onChange);
  return () => window.removeEventListener("hashchange", onChange);
}

const getHash = (): string => window.location.hash;

/** The current route, kept in sync with the URL hash. Re-renders only when the hash changes. */
export function useRoute(): NavItem {
  const hash = useSyncExternalStore(subscribe, getHash, () => "");
  return routeFromHash(hash);
}
