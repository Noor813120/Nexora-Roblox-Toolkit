import { Gauge } from "lucide-react";
import { PlannedPage } from "../../components/layout/PlannedPage";

export function PerformancePage() {
  return (
    <PlannedPage
      title="Performance"
      description="CPU and memory for the app and each client, with limits you set."
      icon={Gauge}
      emptyTitle="No measurements yet"
      emptyDescription="Per-client CPU and memory readings and the Resource Guard limits will be here. The Dashboard already shows a snapshot of the computer's total memory."
      phase="Planned for Phase 7"
    />
  );
}
