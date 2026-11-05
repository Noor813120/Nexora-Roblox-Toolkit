import { Users } from "lucide-react";
import { PlannedPage } from "../../components/layout/PlannedPage";

export function AccountsPage() {
  return (
    <PlannedPage
      title="Accounts"
      description="Roblox account profiles, labels and per-account launch settings."
      icon={Users}
      emptyTitle="No accounts yet"
      emptyDescription="Profiles with labels, categories and launch settings will live here. Signing in always happens on Roblox's own page, and passwords are never stored."
      phase="Planned for Phase 3"
    />
  );
}
