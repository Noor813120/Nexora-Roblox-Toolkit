import { Rocket } from "lucide-react";
import { PlannedPage } from "../../components/layout/PlannedPage";

export function LauncherPage() {
  return (
    <PlannedPage
      title="Game Launcher"
      description="Game targets, the launch queue and the Roblox clients that are running."
      icon={Rocket}
      emptyTitle="No clients running"
      emptyDescription="Saved game targets, the launch queue and every running client will appear here. Several clients can run at once; launching the same account into the same game twice will be flagged first."
      phase="Planned for Phase 4"
    />
  );
}
