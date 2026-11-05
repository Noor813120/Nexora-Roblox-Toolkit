const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/**
 * Formats a byte count the way Windows Task Manager does: 1024-based steps with GB/MB labels.
 * Returns an em dash for values that are not a finite, non-negative number.
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

/** Percentage of `part` in `whole`, clamped to 0-100 and rounded. 0 when `whole` is not positive. */
export function percent(part: number, whole: number): number {
  if (!(whole > 0) || !Number.isFinite(part)) return 0;
  return Math.min(100, Math.max(0, Math.round((part / whole) * 100)));
}

/** A local time such as `14:05:09`, for "snapshot taken at" labels. */
export function formatClock(timestampMs: number): string {
  return new Date(timestampMs).toLocaleTimeString(undefined, { hour12: false });
}
