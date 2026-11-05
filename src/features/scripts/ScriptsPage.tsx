import { FileCode } from "lucide-react";
import { PlannedPage } from "../../components/layout/PlannedPage";

export function ScriptsPage() {
  return (
    <PlannedPage
      title="Script Hub"
      description="Automation scripts, and Luau files for Roblox Studio."
      icon={FileCode}
      emptyTitle="No scripts yet"
      emptyDescription="You will be able to import and organise automation scripts and Luau development files. Imported scripts never run automatically, and Luau here is for Roblox Studio, not for running inside a game client."
      phase="Planned for Phase 6"
    />
  );
}
