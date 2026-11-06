import { Rocket } from "lucide-react";
import { PageLayout } from "../../components/layout/PageLayout";
import { Card } from "../../components/ui/Card";
import { EmptyState } from "../../components/ui/EmptyState";
import { RobloxStatusCard } from "./RobloxStatusCard";

export function LauncherPage() {
  return (
    <PageLayout
      title="Game Launcher"
      description="Game targets, the launch queue and the Roblox clients that are running."
    >
      <div className="space-y-4">
        <RobloxStatusCard />
        <Card>
          <EmptyState
            icon={Rocket}
            title="No clients running"
            description="Saved game targets, the launch queue and every running client will appear here. Several clients can run at once; launching the same account into the same game twice will be flagged first."
            phase="Planned for Phase 4"
          />
        </Card>
      </div>
    </PageLayout>
  );
}
