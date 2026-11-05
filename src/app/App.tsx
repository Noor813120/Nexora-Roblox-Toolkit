import { useEffect, useState } from "react";
import { api, inTauri, type AppInfo } from "../api";

type State =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo }
  | { kind: "error"; message: string };

/**
 * Phase 1 shell. It has exactly one job: prove that the React frontend can call the Rust backend
 * through typed IPC. The sidebar, dashboard and the rest arrive in later phases.
 */
export function App() {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    if (!inTauri()) {
      setState({ kind: "error", message: "Not running inside the RoALT Advanced window." });
      return;
    }
    api
      .appInfo()
      .then((info) => setState({ kind: "ready", info }))
      .catch((e: unknown) => setState({ kind: "error", message: String(e) }));
  }, []);

  return (
    <main className="flex h-full items-center justify-center p-8">
      <section className="w-full max-w-md rounded-xl border border-[var(--border)] bg-[var(--surface)] p-6">
        <h1 className="text-xl font-semibold">RoALT Advanced</h1>
        <p className="mt-1 text-sm text-[var(--muted)]">Phase 1: foundation. No features yet.</p>

        <div className="mt-5 text-sm" aria-live="polite">
          {state.kind === "loading" && <p className="text-[var(--muted)]">Connecting to the backend…</p>}
          {state.kind === "error" && <p className="text-red-400">{state.message}</p>}
          {state.kind === "ready" && (
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
              <dt className="text-[var(--muted)]">Version</dt>
              <dd>{state.info.version}</dd>
              <dt className="text-[var(--muted)]">Platform</dt>
              <dd>
                {state.info.platform} ({state.info.arch})
              </dd>
              <dt className="text-[var(--muted)]">Backend</dt>
              <dd className="text-[var(--accent)]">connected</dd>
            </dl>
          )}
        </div>
      </section>
    </main>
  );
}
