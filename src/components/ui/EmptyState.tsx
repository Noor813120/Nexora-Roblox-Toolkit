import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { StatusPill } from "./StatusPill";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description: string;
  /** Which roadmap phase delivers this, shown as a label so nothing looks broken or hidden. */
  phase?: string;
  /** An optional real action. Do not pass placeholder buttons that do nothing. */
  action?: ReactNode;
  /** A smaller layout for use inside a card. */
  compact?: boolean;
}

/** What a page or panel shows when its feature does not exist yet, or has no data. */
export function EmptyState({ icon: Icon, title, description, phase, action, compact = false }: EmptyStateProps) {
  return (
    <div className={`flex flex-col items-center text-center ${compact ? "py-4" : "py-16"}`}>
      <Icon aria-hidden="true" className="h-8 w-8 text-muted" strokeWidth={1.5} />
      <h2 className="mt-3 text-base font-semibold">{title}</h2>
      <p className="mt-1 max-w-md text-sm text-muted">{description}</p>
      {phase && (
        <div className="mt-3">
          <StatusPill>{phase}</StatusPill>
        </div>
      )}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}
