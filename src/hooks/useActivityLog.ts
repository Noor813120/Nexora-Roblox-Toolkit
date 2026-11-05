import { useCallback, useEffect, useRef, useState } from "react";
import { api, inTauri, type ActivityLog, type Severity } from "../api";
import { describeError } from "../lib/errors";

export type ActivityLogState =
  | { status: "loading" }
  | { status: "offline" }
  | { status: "error"; message: string }
  | {
      status: "ready";
      items: ActivityLog[];
      /** `true` when an older page exists. */
      hasMore: boolean;
      refreshing: boolean;
      loadingMore: boolean;
      /** A failed "load older" does not discard what is already shown. */
      moreError: string | null;
    };

const PAGE_SIZE = 50;

/**
 * Activity entries, newest first, one page at a time. Changing the severity filter starts over;
 * `reload()` refreshes in place; `loadMore()` appends the next older page. Replies that arrive
 * after a newer request started are dropped.
 */
export function useActivityLog(severity: Severity | null) {
  const [state, setState] = useState<ActivityLogState>({ status: "loading" });
  const generation = useRef(0);
  const cursor = useRef<number | null>(null);

  const load = useCallback(
    (keepView: boolean) => {
      const id = ++generation.current;
      if (!inTauri()) {
        setState({ status: "offline" });
        return;
      }
      setState((prev) =>
        keepView && prev.status === "ready" ? { ...prev, refreshing: true, moreError: null } : { status: "loading" },
      );
      api.listActivityLogs({ limit: PAGE_SIZE, severity }).then(
        (page) => {
          if (id !== generation.current) return;
          cursor.current = page.nextBeforeId;
          setState({
            status: "ready",
            items: page.items,
            hasMore: page.nextBeforeId !== null,
            refreshing: false,
            loadingMore: false,
            moreError: null,
          });
        },
        (error: unknown) => {
          if (id === generation.current) setState({ status: "error", message: describeError(error) });
        },
      );
    },
    [severity],
  );

  useEffect(() => {
    load(false);
    return () => {
      generation.current += 1;
    };
  }, [load]);

  const loadMore = useCallback(() => {
    const before = cursor.current;
    if (before === null) return;
    const id = generation.current;
    setState((prev) => (prev.status === "ready" ? { ...prev, loadingMore: true, moreError: null } : prev));
    api.listActivityLogs({ limit: PAGE_SIZE, severity, beforeId: before }).then(
      (page) => {
        if (id !== generation.current) return;
        cursor.current = page.nextBeforeId;
        setState((prev) => {
          if (prev.status !== "ready") return prev;
          const known = new Set(prev.items.map((i) => i.id));
          const fresh = page.items.filter((i) => !known.has(i.id));
          return { ...prev, items: [...prev.items, ...fresh], hasMore: page.nextBeforeId !== null, loadingMore: false };
        });
      },
      (error: unknown) => {
        if (id !== generation.current) return;
        setState((prev) =>
          prev.status === "ready" ? { ...prev, loadingMore: false, moreError: describeError(error) } : prev,
        );
      },
    );
  }, [severity]);

  const reload = useCallback(() => load(true), [load]);

  return { state, reload, loadMore };
}
