import { SlidersHorizontal } from "lucide-react";
import { api } from "../../api";
import { PageLayout } from "../../components/layout/PageLayout";
import { Card } from "../../components/ui/Card";
import { EmptyState } from "../../components/ui/EmptyState";
import { useBackend } from "../../hooks/useBackend";

export function SettingsPage() {
  const { state } = useBackend(api.appInfo);

  return (
    <PageLayout title="Settings" description="Preferences and information about this app.">
      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="About">
          {state.status === "loading" && <p className="text-sm text-muted">Reading build information…</p>}
          {state.status === "offline" && (
            <p className="text-sm text-muted">Build information is only available inside the app window.</p>
          )}
          {state.status === "error" && <p className="text-sm text-danger">{state.message}</p>}
          {state.status === "ready" && (
            <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
              <dt className="text-muted">Name</dt>
              <dd>{state.data.name}</dd>
              <dt className="text-muted">Version</dt>
              <dd>{state.data.version}</dd>
              <dt className="text-muted">Platform</dt>
              <dd>{state.data.platform}</dd>
              <dt className="text-muted">Architecture</dt>
              <dd>{state.data.arch}</dd>
            </dl>
          )}
        </Card>
        <Card title="Preferences">
          <EmptyState
            compact
            icon={SlidersHorizontal}
            title="No preferences yet"
            description="Settings will be saved once the database exists."
            phase="Planned for Phase 3"
          />
        </Card>
      </div>
    </PageLayout>
  );
}
