import { ScrollText } from "lucide-react";
import { PlannedPage } from "../../components/layout/PlannedPage";

export function LogsPage() {
  return (
    <PlannedPage
      title="Activity Logs"
      description="What the app and its tasks did."
      icon={ScrollText}
      emptyTitle="No activity recorded"
      emptyDescription="A history of launches, retries and errors will be listed here once activity logging is built. Nothing is being recorded yet."
      phase="Planned for Phase 3"
    />
  );
}
