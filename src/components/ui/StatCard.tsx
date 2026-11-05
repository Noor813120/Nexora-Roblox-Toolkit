import type { ReactNode } from "react";
import { Card } from "./Card";
import { StatusPill, type Tone } from "./StatusPill";

interface StatCardProps {
  label: string;
  /** The headline value. Use "—" when there is no real value to show. */
  value: string;
  /** Small label beside the title that says where the number stands, e.g. "Not available yet". */
  badge?: { text: string; tone?: Tone };
  /** One or two lines of explanation: what the number is, or why it is missing. */
  detail?: ReactNode;
  /** Optional extra content under the detail, such as a usage bar. */
  children?: ReactNode;
}

/** A dashboard tile: label, one headline value, and an honest note about where the value comes from. */
export function StatCard({ label, value, badge, detail, children }: StatCardProps) {
  return (
    <Card title={label} actions={badge && <StatusPill tone={badge.tone}>{badge.text}</StatusPill>}>
      <p className="text-2xl font-semibold tabular-nums">{value}</p>
      {detail && <p className="mt-1 text-xs text-muted">{detail}</p>}
      {children}
    </Card>
  );
}
