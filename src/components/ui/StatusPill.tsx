import type { ReactNode } from "react";

export type Tone = "ok" | "neutral" | "warn" | "danger";

const TONES: Record<Tone, string> = {
  ok: "border-ok/40 text-ok",
  neutral: "border-line text-muted",
  warn: "border-warn/40 text-warn",
  danger: "border-danger/40 text-danger",
};

/**
 * A short status label. The meaning is always in the text; colour only reinforces it, so the pill
 * reads correctly for people who cannot tell the colours apart.
 */
export function StatusPill({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span className={`inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium ${TONES[tone]}`}>
      {children}
    </span>
  );
}
