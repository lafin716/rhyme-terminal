import { describe, expect, it } from "vitest";
import { formatBytes, isUpdateBusy, progressPercent } from "./updater";

describe("progressPercent", () => {
  it("is null until the download size is known", () => {
    expect(progressPercent(null)).toBeNull();
    expect(progressPercent({ downloaded: 10 })).toBeNull();
    expect(progressPercent({ downloaded: 10, total: 0 })).toBeNull();
  });
  it("floors and clamps to 0–100", () => {
    expect(progressPercent({ downloaded: 1, total: 3 })).toBe(33);
    expect(progressPercent({ downloaded: 50, total: 40 })).toBe(100);
  });
});

describe("formatBytes", () => {
  it("picks a readable unit", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(25 * 1024 * 1024)).toBe("25 MB");
  });
});

describe("isUpdateBusy", () => {
  it("blocks re-entry only while work is running", () => {
    expect(isUpdateBusy("checking")).toBe(true);
    expect(isUpdateBusy("downloading")).toBe(true);
    expect(isUpdateBusy("installing")).toBe(true);
    expect(isUpdateBusy("available")).toBe(false);
    expect(isUpdateBusy("error")).toBe(false);
  });
});
