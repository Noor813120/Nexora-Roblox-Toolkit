import { useState } from "react";
import { api } from "../../api";
import { PageLayout } from "../../components/layout/PageLayout";
import { Card } from "../../components/ui/Card";
import { useBackend } from "../../hooks/useBackend";
import { DatabaseCard } from "./DatabaseCard";
import { PreferencesCard } from "./PreferencesCard";

export function SettingsPage() {
  const { state } = useBackend(api.appInfo);
  // Bumped after a restore so the preferences are read again from the restored database.
  const [dataVersion, setDataVersion] = useState(0);

  return (
    <PageLayout title="Settings" description="Preferences, stored data and information about this app.">
      <div className="grid gap-4 lg:grid-cols-2">
        <PreferencesCard key={dataVersion} />
        <DatabaseCard onRestored={() => setDataVersion((v) => v + 1)} />
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
      </div>
    </PageLayout>
  );
}
