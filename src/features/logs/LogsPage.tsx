import { RefreshCw, ScrollText, Trash2 } from "lucide-react";
import { useId, useState } from "react";
import { api, inTauri, type ActivityLog, type Severity } from "../../api";
import { PageLayout } from "../../components/layout/PageLayout";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import { EmptyState } from "../../components/ui/EmptyState";
import { useAction } from "../../hooks/useAction";
import { useActivityLog } from "../../hooks/useActivityLog";
import { formatDateTime } from "../../lib/format";
import { SEVERITIES, SeverityPill, severityLabel } from "./SeverityPill";

function prettyMetadata(json: string): string {
  try {
    return JSON.stringify(JSON.parse(json), null, 2);
  } catch {
    return json;
  }
}

function LogTable({ items }: { items: ActivityLog[] }) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left text-sm">
        <caption className="sr-only">Activity entries, newest first</caption>
        <thead>
          <tr className="border-b border-line text-xs text-muted">
            <th scope="col" className="py-2 pr-4 font-medium">Time</th>
            <th scope="col" className="py-2 pr-4 font-medium">Severity</th>
            <th scope="col" className="py-2 pr-4 font-medium">Event</th>
            <th scope="col" className="py-2 font-medium">Message</th>
          </tr>
        </thead>
        <tbody>
          {items.map((log) => (
            <tr key={log.id} className="border-b border-line/60 align-top last:border-0">
              <td className="whitespace-nowrap py-2 pr-4 tabular-nums text-muted">
                <time dateTime={new Date(log.createdAt).toISOString()}>{formatDateTime(log.createdAt)}</time>
              </td>
              <td className="py-2 pr-4">
                <SeverityPill severity={log.severity} />
              </td>
              <td className="py-2 pr-4 font-mono text-xs text-muted">{log.eventType}</td>
              <td className="py-2">
                <p className="whitespace-pre-wrap break-words">{log.message}</p>
                {log.metadataJson && (
                  <details className="mt-1">
                    <summary className="cursor-pointer text-xs text-muted">Details</summary>
                    <pre className="mt-1 overflow-x-auto rounded-md bg-raised p-2 text-xs">{prettyMetadata(log.metadataJson)}</pre>
                  </details>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** Removes entries older than N days, after an explicit confirmation step. */
function ClearOld({ onCleared }: { onCleared: () => void }) {
  const inputId = useId();
  const [open, setOpen] = useState(false);
  const [days, setDays] = useState("30");
  const [removed, setRemoved] = useState<number | null>(null);
  const clear = useAction(api.clearOldActivityLogs);
  const parsed = Number(days);
  const valid = Number.isInteger(parsed) && parsed >= 1 && parsed <= 3650;

  const confirm = async () => {
    const count = await clear.run(parsed);
    if (count !== undefined) {
      setRemoved(count);
      setOpen(false);
      onCleared();
    }
  };

  return (
    <Card title="Clean up">
      {!open ? (
        <div className="flex flex-wrap items-center gap-3">
          <Button onClick={() => { setOpen(true); setRemoved(null); clear.reset(); }} disabled={!inTauri()}>
            <Trash2 aria-hidden="true" className="h-4 w-4" />
            Remove old entries…
          </Button>
          {removed !== null && (
            <p role="status" className="text-sm text-muted">
              {removed === 0 ? "Nothing was old enough to remove." : `Removed ${removed} ${removed === 1 ? "entry" : "entries"}.`}
            </p>
          )}
          <p className="text-sm text-muted">The history is also trimmed automatically; see Settings.</p>
        </div>
      ) : (
        <div className="flex flex-wrap items-end gap-3">
          <div>
            <label htmlFor={inputId} className="block text-xs text-muted">Remove entries older than (days)</label>
            <input
              id={inputId}
              type="number"
              min={1}
              max={3650}
              value={days}
              onChange={(e) => setDays(e.target.value)}
              className="mt-1 w-28 rounded-md border border-line bg-canvas px-2 py-1.5 text-sm"
            />
          </div>
          <Button variant="primary" onClick={confirm} disabled={!valid || clear.pending}>
            Remove entries older than {valid ? parsed : "…"} days
          </Button>
          <Button onClick={() => setOpen(false)} disabled={clear.pending}>Cancel</Button>
          {!valid && <p className="text-sm text-warn">Enter a whole number from 1 to 3650.</p>}
        </div>
      )}
      {clear.state.status === "error" && (
        <p role="alert" className="mt-2 text-sm text-danger">{clear.state.message}</p>
      )}
    </Card>
  );
}

export function LogsPage() {
  const filterId = useId();
  const [severity, setSeverity] = useState<Severity | null>(null);
  const { state, reload, loadMore } = useActivityLog(severity);

  return (
    <PageLayout
      title="Activity Logs"
      description="What the app did, newest first. Stored on this computer."
      actions={
        <Button onClick={reload} disabled={state.status === "offline" || state.status === "loading"}>
          <RefreshCw aria-hidden="true" className="h-4 w-4" />
          Refresh
        </Button>
      }
    >
      <Card
        title="Entries"
        actions={
          <div className="flex items-center gap-2">
            <label htmlFor={filterId} className="text-xs text-muted">Severity</label>
            <select
              id={filterId}
              value={severity ?? ""}
              onChange={(e) => setSeverity(e.target.value === "" ? null : (e.target.value as Severity))}
              className="rounded-md border border-line bg-canvas px-2 py-1 text-sm"
            >
              <option value="">All</option>
              {SEVERITIES.map((s) => (
                <option key={s} value={s}>{severityLabel(s)}</option>
              ))}
            </select>
          </div>
        }
      >
        {state.status === "loading" && <p className="py-6 text-center text-sm text-muted">Loading activity…</p>}
        {state.status === "offline" && (
          <p className="py-6 text-center text-sm text-muted">
            Activity is only available inside the Nexora window.
          </p>
        )}
        {state.status === "error" && (
          <div role="alert" className="py-6 text-center">
            <p className="text-sm text-danger">{state.message}</p>
            <div className="mt-3">
              <Button onClick={reload}>Try again</Button>
            </div>
          </div>
        )}
        {state.status === "ready" && state.items.length === 0 && (
          <EmptyState
            compact
            icon={ScrollText}
            title={severity ? `No ${severityLabel(severity).toLowerCase()} entries` : "No activity recorded"}
            description={
              severity
                ? "Nothing at this severity has been recorded. Choose All to see everything."
                : "Entries appear here as the app records events, such as starting up or backing up the database."
            }
          />
        )}
        {state.status === "ready" && state.items.length > 0 && (
          <>
            <LogTable items={state.items} />
            <div className="mt-3 flex items-center gap-3">
              {state.hasMore && (
                <Button onClick={loadMore} disabled={state.loadingMore}>
                  {state.loadingMore ? "Loading…" : "Load older entries"}
                </Button>
              )}
              {!state.hasMore && <p className="text-xs text-muted">That is everything that is stored.</p>}
              {state.moreError && <p role="alert" className="text-sm text-danger">{state.moreError}</p>}
            </div>
          </>
        )}
      </Card>
      <div className="mt-4">
        <ClearOld onCleared={reload} />
      </div>
    </PageLayout>
  );
}
