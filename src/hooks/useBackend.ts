import { useCallback, useEffect, useRef, useState } from "react";
import { inTauri } from "../api";
import { describeError } from "../lib/errors";

export type BackendState<T> =
  | { status: "loading" }
  /** Not inside the app window (for example the page opened in a plain browser tab). */
  | { status: "offline" }
  | { status: "error"; message: string }
  /** `at` is when the data was read, in ms since the epoch. */
  | { status: "ready"; data: T; at: number };

/**
 * Loads one backend value when the component mounts and again on `reload()`. It never polls.
 *
 * `load` is read through a ref, so passing an inline function does not retrigger the request. A
 * response that arrives after a newer request started, or after unmount, is dropped.
 */
export function useBackend<T>(load: () => Promise<T>): {
  state: BackendState<T>;
  reload: () => void;
} {
  const [state, setState] = useState<BackendState<T>>({ status: "loading" });
  const loadRef = useRef(load);
  loadRef.current = load;
  const latest = useRef(0);

  const run = useCallback(() => {
    const id = ++latest.current;
    if (!inTauri()) {
      setState({ status: "offline" });
      return;
    }
    // Keep showing the old data while refreshing; only the first load shows a loading state.
    setState((prev) => (prev.status === "ready" ? prev : { status: "loading" }));
    loadRef.current().then(
      (data) => {
        if (id === latest.current) setState({ status: "ready", data, at: Date.now() });
      },
      (error: unknown) => {
        if (id === latest.current) setState({ status: "error", message: describeError(error) });
      },
    );
  }, []);

  useEffect(() => {
    run();
    return () => {
      latest.current += 1; // invalidates any request still in flight
    };
  }, [run]);

  return { state, reload: run };
}
