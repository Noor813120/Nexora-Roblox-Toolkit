import { RefreshCw } from "lucide-react";
import { api, type AppInfo, type SystemInfo } from "../../api";
import { hrefFor, NAV_ITEMS } from "../../app/navigation";
import { PageLayout } from "../../components/layout/PageLayout";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import { StatCard } from "../../components/ui/StatCard";
import { StatusPill } from "../../components/ui/StatusPill";
import { useBackend, type BackendState } from "../../hooks/useBackend";
import { formatBytes, formatClock, percent } from "../../lib/format";
import { loadRecent, RecentActivity } from "./RecentActivity";

const OFFLINE_NOTE = "This page is not running inside the Nexora window, so there is no backend to ask.";

/** Application status: the one card whose data comes from the backend's `app_info`. */
function ApplicationCard({ state }: { state: BackendState<AppInfo> }) {
  switch (state.status) {
    case "loading":
      return <StatCard label="Application" value="…" detail="Connecting to the backend…" />;
    case "offline":
      return <StatCard label="Application" value="Not connected" badge={{ text: "Offline", tone: "warn" }} detail={OFFLINE_NOTE} />;
    case "error":
      return <StatCard label="Application" value="Error" badge={{ text: "Failed", tone: "danger" }} detail={state.message} />;
    case "ready":
      return (
        <StatCard
          label="Application"
          value="Running"
          badge={{ text: "Connected", tone: "ok" }}
          detail={`Version ${state.data.version} · ${state.data.platform} ${state.data.arch}`}
        />
      );
  }
}

/** CPU: only the core count is real. Live usage is not measured until Phase 7, and the card says so. */
function CpuCard({ state }: { state: BackendState<SystemInfo> }) {
  const cores = state.status === "ready" ? state.data.logicalCores : null;
  return (
    <StatCard
      label="CPU"
      value={cores === null ? "—" : `${cores} logical ${cores === 1 ? "core" : "cores"}`}
      badge={{ text: "Usage not measured yet", tone: "neutral" }}
      detail="Core count only. Live CPU usage arrives in Phase 7."
    />
  );
}

/** Memory: a one-time snapshot of the whole computer, not of this app or of Roblox. */
function MemoryCard({ state }: { state: BackendState<SystemInfo> }) {
  if (state.status !== "ready") {
    return (
      <StatCard
        label="Memory"
        value="—"
        detail={state.status === "offline" ? OFFLINE_NOTE : state.status === "error" ? state.message : "Reading…"}
      />
    );
  }
  const memory = state.data.memory;
  if (memory === null) {
    return (
      <StatCard
        label="Memory"
        value="—"
        badge={{ text: "Unavailable", tone: "neutral" }}
        detail="Memory readings are only implemented for Windows."
      />
    );
  }
  const used = percent(memory.usedBytes, memory.totalBytes);
  return (
    <StatCard
      label="Memory"
      value={`${formatBytes(memory.usedBytes)} of ${formatBytes(memory.totalBytes)}`}
      badge={{ text: "Snapshot", tone: "neutral" }}
      detail={`Whole computer, read at ${formatClock(state.at)}. Not this app or Roblox.`}
    >
      <div
        role="progressbar"
        aria-label="Computer memory in use"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={used}
        className="mt-3 h-1.5 overflow-hidden rounded-full bg-raised"
      >
        <div className="h-full rounded-full bg-accent" style={{ width: `${used}%` }} />
      </div>
      <p className="mt-1 text-xs tabular-nums text-muted">{used}% in use</p>
    </StatCard>
  );
}

export function DashboardPage() {
  const app = useBackend(api.appInfo);
  const system = useBackend(api.systemInfo);
  const recent = useBackend(loadRecent);
  const links = NAV_ITEMS.filter((item) => item.id !== "dashboard");

  return (
    <PageLayout
      title="Dashboard"
      description="Status of Nexora and this computer."
      actions={
        <Button
          onClick={() => {
            system.reload();
            recent.reload();
          }}
          disabled={system.state.status === "offline"}
        >
          <RefreshCw aria-hidden="true" className="h-4 w-4" />
          Refresh
        </Button>
      }
    >
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <ApplicationCard state={app.state} />
        <StatCard
          label="Roblox clients"
          value="—"
          badge={{ text: "Not available yet", tone: "neutral" }}
          detail="Client tracking arrives in Phase 4. This is not a count of zero."
        />
        <CpuCard state={system.state} />
        <MemoryCard state={system.state} />
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <Card title="Automation">
          <div className="flex items-center gap-2">
            <StatusPill>Not available yet</StatusPill>
          </div>
          <p className="mt-2 text-sm text-muted">The automation engine arrives in Phase 5. Nothing is running or scheduled.</p>
        </Card>
        <RecentActivity state={recent.state} reload={recent.reload} />
      </div>

      <div className="mt-4">
        <Card title="Quick navigation">
          <ul className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            {links.map((item) => {
              const Icon = item.icon;
              return (
                <li key={item.id}>
                  <a
                    href={hrefFor(item)}
                    className="ui-transition flex items-start gap-3 rounded-md border border-line p-3 hover:bg-raised"
                  >
                    <Icon aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-accent" strokeWidth={1.75} />
                    <span>
                      <span className="block text-sm font-medium">{item.label}</span>
                      <span className="block text-xs text-muted">{item.description}</span>
                    </span>
                  </a>
                </li>
              );
            })}
          </ul>
        </Card>
      </div>
    </PageLayout>
  );
}
