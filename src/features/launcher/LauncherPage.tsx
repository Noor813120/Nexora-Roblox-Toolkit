
import { useCallback, useEffect, useState } from "react";
import { RefreshCw, Rocket } from "lucide-react";
import { api, type RunningRobloxClients } from "../../api";
import { PageLayout } from "../../components/layout/PageLayout";
import { Button } from "../../components/ui/Button";
import { Card } from "../../components/ui/Card";
import { EmptyState } from "../../components/ui/EmptyState";
import { StatusPill } from "../../components/ui/StatusPill";
import { RobloxStatusCard } from "./RobloxStatusCard";

type ClientState =
  | { status: "loading" }
  | { status: "ready"; data: RunningRobloxClients }
  | { status: "error" };

export function LauncherPage() {
  const [clientState, setClientState] = useState<ClientState>({
    status: "loading",
  });

  const refreshClients = useCallback(async () => {
    setClientState({ status: "loading" });

    try {
      const data = await api.getRunningRobloxClients();
      setClientState({ status: "ready", data });
    } catch {
      setClientState({ status: "error" });
    }
  }, []);

  useEffect(() => {
    void refreshClients();
  }, [refreshClients]);

  const clients =
    clientState.status === "ready" ? clientState.data.clients : [];

  return (
    <PageLayout
      title="Game Launcher"
      description="Game targets, the launch queue and the Roblox clients that are running."
    >
      <div className="space-y-4">
        <RobloxStatusCard />

        <Card
          title="Running Roblox clients"
          actions={
            <Button
              onClick={refreshClients}
              disabled={clientState.status === "loading"}
              aria-label="Refresh running clients"
            >
              <RefreshCw
                aria-hidden="true"
                className={`h-4 w-4 ${
                  clientState.status === "loading" ? "animate-spin" : ""
                }`}
              />
              Refresh
            </Button>
          }
        >
          {clientState.status === "loading" && (
            <p role="status" className="py-4 text-sm text-muted">
              Checking for running Roblox clients…
            </p>
          )}

          {clientState.status === "error" && (
            <div role="alert" className="py-4 text-sm text-danger">
              Could not check running Roblox clients. Try refreshing.
            </div>
          )}

          {clientState.status === "ready" &&
            clientState.data.status === "unsupported" && (
              <div className="py-4">
                <StatusPill tone="warn">Unsupported platform</StatusPill>
                <p className="mt-2 text-sm text-muted">
                  Running Roblox process detection is currently available only
                  on Windows.
                </p>
              </div>
            )}

          {clientState.status === "ready" &&
            clientState.data.status === "available" &&
            clients.length === 0 && (
              <div className="py-4">
                <StatusPill tone="neutral">No matching processes</StatusPill>
                <p className="mt-2 text-sm text-muted">
                  No Roblox Player processes were found in the latest process
                  snapshot.
                </p>
              </div>
            )}

          {clientState.status === "ready" &&
            clientState.data.status === "available" &&
            clients.length > 0 && (
              <div className="space-y-3">
                <div className="flex flex-wrap items-center gap-2">
                  <StatusPill tone="ok">
                    {clients.length} {clients.length === 1 ? "client" : "clients"} detected
                  </StatusPill>
                  <span className="text-xs text-muted">
                    Latest process snapshot
                  </span>
                </div>

                <ul className="divide-y divide-line">
                  {clients.map((client) => (
                    <li
                      key={client.processId}
                      className="flex flex-wrap items-center justify-between gap-3 py-3"
                    >
                      <div className="min-w-0">
                        <p className="break-all text-sm font-medium text-ink">
                          {client.executableName}
                        </p>
                        <p className="mt-1 text-xs text-muted">
                          Process ID: {client.processId}
                        </p>
                      </div>
                      <StatusPill tone="ok">Detected</StatusPill>
                    </li>
                  ))}
                </ul>
              </div>
            )}

          <p className="mt-4 border-t border-line pt-3 text-xs text-muted">
            Detection only confirms that a matching process was present in the
            snapshot. It does not verify account identity, login state, game
            activity, or process health.
          </p>
        </Card>

        <Card>
          <EmptyState
            icon={Rocket}
            title="Game targets and launch queue"
            description="Saved game targets and queued launches are not implemented yet."
            phase="Planned for Phase 4"
          />
        </Card>
      </div>
    </PageLayout>
  );
}
