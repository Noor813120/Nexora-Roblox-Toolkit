import { Activity } from "lucide-react";
import { api, type ActivityLog } from "../../api";
import { Card } from "../../components/ui/Card";
import { EmptyState } from "../../components/ui/EmptyState";
import type { BackendState } from "../../hooks/useBackend";
import { formatDateTime } from "../../lib/format";
import { SeverityPill } from "../logs/SeverityPill";

const COUNT_KEY = "ui.dashboard_activity_count";
const FALLBACK_COUNT = 8;

/**
 * The newest entries, as many as the "dashboard_activity_count" setting says. Two small calls,
 * one after the other, made once when the Dashboard opens; nothing polls.
 */
export async function loadRecent(): Promise<ActivityLog[]> {
  const settings = await api.listSettings();
  const wanted = Number(settings.find((s) => s.key === COUNT_KEY)?.value);
  const limit = Number.isInteger(wanted) && wanted > 0 ? wanted : FALLBACK_COUNT;
  return (await api.listActivityLogs({ limit })).items;
}

export function RecentActivity({ state, reload }: { state: BackendState<ActivityLog[]>; reload: () => void }) {
  return (
    <Card title="Recent activity">
      {state.status === "loading" && <p className="text-sm text-muted">Loading activity…</p>}
      {state.status === "offline" && (
        <p className="text-sm text-muted">Activity is only available inside the RoALT Advanced window.</p>
      )}
      {state.status === "error" && (
        <div role="alert">
          <p className="text-sm text-danger">{state.message}</p>
          <button type="button" onClick={reload} className="mt-2 text-sm text-accent underline">
            Try again
          </button>
        </div>
      )}
      {state.status === "ready" && state.data.length === 0 && (
        <EmptyState
          compact
          icon={Activity}
          title="No activity yet"
          description="Events the app records, such as starting up, appear here."
        />
      )}
      {state.status === "ready" && state.data.length > 0 && (
        <>
          <ul className="divide-y divide-line/60">
            {state.data.map((log) => (
              <li key={log.id} className="flex items-start gap-3 py-2 first:pt-0">
                <SeverityPill severity={log.severity} />
                <div className="min-w-0 flex-1">
                  <p className="break-words text-sm">{log.message}</p>
                  <p className="text-xs tabular-nums text-muted">
                    <time dateTime={new Date(log.createdAt).toISOString()}>{formatDateTime(log.createdAt)}</time>
                  </p>
                </div>
              </li>
            ))}
          </ul>
          <a href="#/logs" className="mt-3 inline-block text-sm text-accent underline">
            View all activity
          </a>
        </>
      )}
    </Card>
  );
}
