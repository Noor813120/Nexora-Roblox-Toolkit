import type { LucideIcon } from "lucide-react";
import { Card } from "../ui/Card";
import { EmptyState } from "../ui/EmptyState";
import { PageLayout } from "./PageLayout";

interface PlannedPageProps {
  title: string;
  description: string;
  icon: LucideIcon;
  emptyTitle: string;
  emptyDescription: string;
  /** e.g. "Planned for Phase 4". Always shown, so an unbuilt section is labelled, not silent. */
  phase: string;
}

/**
 * A page whose feature is not built yet. It shows what will live here and when, with no buttons or
 * fields that look usable but do nothing.
 */
export function PlannedPage({ title, description, icon, emptyTitle, emptyDescription, phase }: PlannedPageProps) {
  return (
    <PageLayout title={title} description={description}>
      <Card>
        <EmptyState icon={icon} title={emptyTitle} description={emptyDescription} phase={phase} />
      </Card>
    </PageLayout>
  );
}
