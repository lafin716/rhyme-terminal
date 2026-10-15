import { describe, expect, it } from "vitest";
import { formatUsagePercent, formatUsageReset } from "./usage-status";

describe("formatUsagePercent", () => {
  it("shows a placeholder when no data source is connected", () => {
    expect(formatUsagePercent({ percentUsed: null, resetsAt: null })).toBe("—%");
  });

  it("rounds to the nearest whole percent", () => {
    expect(formatUsagePercent({ percentUsed: 42.6, resetsAt: null })).toBe("43%");
  });

  it("clamps out-of-range values into 0-100", () => {
    expect(formatUsagePercent({ percentUsed: -5, resetsAt: null })).toBe("0%");
    expect(formatUsagePercent({ percentUsed: 140, resetsAt: null })).toBe("100%");
  });
});

describe("formatUsageReset", () => {
  const now = Date.parse("2026-09-03T12:00:00Z");

  it("shows a placeholder when no data source is connected", () => {
    expect(formatUsageReset({ percentUsed: null, resetsAt: null }, now)).toBe("연동 필요");
  });

  it("reports hours and minutes remaining", () => {
    const resetsAt = now + (2 * 60 + 14) * 60_000;
    expect(formatUsageReset({ percentUsed: 50, resetsAt }, now)).toBe("2시간 14분 후 초기화");
  });

  it("reports minutes only under an hour", () => {
    const resetsAt = now + 9 * 60_000;
    expect(formatUsageReset({ percentUsed: 50, resetsAt }, now)).toBe("9분 후 초기화");
  });

  it("treats a past reset time as already reset", () => {
    expect(formatUsageReset({ percentUsed: 50, resetsAt: now - 1000 }, now)).toBe("초기화됨");
  });
});
