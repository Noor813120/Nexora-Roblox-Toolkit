import type { Severity } from "../../api";
import { StatusPill, type Tone } from "../../components/ui/StatusPill";

const LABEL: Record<Severity, string> = { debug: "Debug", info: "Info", warning: "Warning", error: "Error" };
const TONE: Record<Severity, Tone> = { debug: "neutral", info: "neutral", warning: "warn", error: "danger" };

export const SEVERITIES: readonly Severity[] = ["error", "warning", "info", "debug"];
export const severityLabel = (s: Severity) => LABEL[s];

/** The word is always shown; colour only reinforces it. */
export function SeverityPill({ severity }: { severity: Severity }) {
  return <StatusPill tone={TONE[severity]}>{LABEL[severity]}</StatusPill>;
}
