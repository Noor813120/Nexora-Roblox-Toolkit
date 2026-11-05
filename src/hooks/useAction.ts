import { useCallback, useEffect, useRef, useState } from "react";
import { describeError } from "../lib/errors";

export type ActionState = { status: "idle" } | { status: "pending" } | { status: "done" } | { status: "error"; message: string };

/**
 * Wraps a write operation (save, delete, backup...). `run()` resolves to the result, or to
 * `undefined` when the call failed; the failure is kept in `state` as readable text. A second call
 * while one is running is ignored, so a double click cannot send two writes.
 */
export function useAction<A extends unknown[], R>(action: (...args: A) => Promise<R>) {
  const [state, setState] = useState<ActionState>({ status: "idle" });
  const actionRef = useRef(action);
  actionRef.current = action;
  const busy = useRef(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const run = useCallback(async (...args: A): Promise<R | undefined> => {
    if (busy.current) return undefined;
    busy.current = true;
    if (mounted.current) setState({ status: "pending" });
    try {
      const result = await actionRef.current(...args);
      if (mounted.current) setState({ status: "done" });
      return result;
    } catch (error) {
      if (mounted.current) setState({ status: "error", message: describeError(error) });
      return undefined;
    } finally {
      busy.current = false;
    }
  }, []);

  const reset = useCallback(() => setState({ status: "idle" }), []);

  return { state, run, reset, pending: state.status === "pending" };
}
