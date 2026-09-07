import { describe, expect, it } from "vitest";
import { titlebarInsets } from "./titlebar-layout";

describe("titlebar control reservations", () => {
  it("reserves window controls on a full-width tab row", () => {
    expect(titlebarInsets({ top: 0, left: 224, right: 1200, width: 976 }, 1200, 220)).toEqual({ left: 0, right: 172 });
  });
  it("reserves the menu when the sidebar is hidden", () => {
    expect(titlebarInsets({ top: 0, left: 0, right: 1200, width: 1200 }, 1200, 102)).toEqual({ left: 102, right: 172 });
  });
  it("keeps lower split panes and tabs beside the Explorer unchanged", () => {
    expect(titlebarInsets({ top: 200, left: 0, right: 1200, width: 1200 }, 1200, 102)).toEqual({ left: 0, right: 0 });
    expect(titlebarInsets({ top: 0, left: 224, right: 896, width: 672 }, 1200, 220)).toEqual({ left: 0, right: 0 });
  });
  it("reserves intersections across multiple narrow split panes", () => {
    expect(titlebarInsets({ top: 0, left: 900, right: 1100, width: 200 }, 1200, 220)).toEqual({ left: 0, right: 72 });
    expect(titlebarInsets({ top: 0, left: 1104, right: 1200, width: 96 }, 1200, 220)).toEqual({ left: 0, right: 96 });
  });
  it("never produces a negative tab width when controls consume the row", () => {
    expect(titlebarInsets({ top: 0, left: 0, right: 200, width: 200 }, 200, 102)).toEqual({ left: 102, right: 98 });
  });
});
