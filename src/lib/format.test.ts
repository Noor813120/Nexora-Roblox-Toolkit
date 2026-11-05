import { describe, expect, it } from "vitest";
import { formatDateTime, formatBytes, formatClock, percent } from "./format";

describe("formatBytes", () => {
  it("uses 1024-based steps like Task Manager", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1023)).toBe("1023 B");
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(16 * 1024 ** 3)).toBe("16.0 GB");
    expect(formatBytes(1.5 * 1024 ** 3)).toBe("1.5 GB");
  });

  it("drops the decimal for large values and caps at TB", () => {
    expect(formatBytes(512 * 1024 ** 2)).toBe("512 MB");
    expect(formatBytes(3 * 1024 ** 4)).toBe("3.0 TB");
    expect(formatBytes(5000 * 1024 ** 4)).toBe("5000 TB");
  });

  it("returns an em dash for values that make no sense", () => {
    for (const bad of [-1, NaN, Infinity]) expect(formatBytes(bad)).toBe("—");
  });
});

describe("percent", () => {
  it("rounds and clamps to 0-100", () => {
    expect(percent(10, 16)).toBe(63);
    expect(percent(0, 16)).toBe(0);
    expect(percent(20, 16)).toBe(100);
    expect(percent(-5, 16)).toBe(0);
  });

  it("is 0 when the whole is zero, negative or not a number", () => {
    expect(percent(5, 0)).toBe(0);
    expect(percent(5, -1)).toBe(0);
    expect(percent(5, NaN)).toBe(0);
    expect(percent(NaN, 10)).toBe(0);
  });
});

describe("formatClock", () => {
  it("formats a 24-hour clock time", () => {
    expect(formatClock(Date.now())).toMatch(/^\d{1,2}:\d{2}:\d{2}$/);
  });
});

describe("formatDateTime", () => {
  it("includes the date and the time of day", () => {
    const text = formatDateTime(new Date(2026, 10, 4, 14, 5, 9).getTime());
    expect(text).toMatch(/2026/);
    expect(text).toMatch(/14:05:09/);
  });

  it("gives an em dash for a value that is not a time", () => {
    expect(formatDateTime(Number.NaN)).toBe("—");
    expect(formatDateTime(Number.POSITIVE_INFINITY)).toBe("—");
  });
});
