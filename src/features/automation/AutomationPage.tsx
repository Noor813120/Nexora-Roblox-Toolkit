import { Workflow } from "lucide-react";
import { PlannedPage } from "../../components/layout/PlannedPage";

export function AutomationPage() {
  return (
    <PlannedPage
      title="Automation"
      description="Auto-join, reconnect and idle alerts."
      icon={Workflow}
      emptyTitle="Automation is not available yet"
      emptyDescription="Auto-join and auto-reconnect with retry limits will be set up here. Idle alerts only notify you; they never send input to keep a client from being kicked for inactivity."
      phase="Planned for Phase 5"
    />
  );
}
