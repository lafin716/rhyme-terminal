import { describe, it, expect } from "vitest";
import {
  MAX_SCALE,
  MIN_SCALE,
  clampOffset,
  fitScale,
  normalizeRotation,
  rotatedSize,
  stepScale,
  zoomAround,
} from "./image-view";

describe("normalizeRotation", () => {
  it("wraps into 0..270 in quarter turns", () => {
    expect(normalizeRotation(360)).toBe(0);
    expect(normalizeRotation(450)).toBe(90);
    expect(normalizeRotation(-90)).toBe(270);
  });
});

describe("rotatedSize", () => {
  it("swaps width and height for quarter turns only", () => {
    expect(rotatedSize({ width: 40, height: 10 }, 90)).toEqual({ width: 10, height: 40 });
    expect(rotatedSize({ width: 40, height: 10 }, 180)).toEqual({ width: 40, height: 10 });
  });
});

describe("fitScale", () => {
  it("shrinks a large image to fit the tighter axis", () => {
    expect(fitScale({ width: 2000, height: 1000 }, { width: 1000, height: 1000 })).toBe(0.5);
  });

  it("does not enlarge a small image", () => {
    expect(fitScale({ width: 100, height: 50 }, { width: 1000, height: 1000 })).toBe(1);
  });

  it("accounts for rotation", () => {
    expect(fitScale({ width: 2000, height: 500 }, { width: 1000, height: 500 }, 90)).toBe(0.25);
  });

  it("falls back to 1 for unknown sizes", () => {
    expect(fitScale({ width: 0, height: 0 }, { width: 100, height: 100 })).toBe(1);
  });
});

describe("stepScale", () => {
  it("moves to the neighbouring preset", () => {
    expect(stepScale(1, 1)).toBe(1.25);
    expect(stepScale(1, -1)).toBe(0.75);
    expect(stepScale(0.8, -1)).toBe(0.75);
    expect(stepScale(0.8, 1)).toBe(1);
  });

  it("stops at the limits", () => {
    expect(stepScale(MAX_SCALE, 1)).toBe(MAX_SCALE);
    expect(stepScale(MIN_SCALE, -1)).toBe(MIN_SCALE);
  });
});

describe("zoomAround", () => {
  it("keeps the anchored point still", () => {
    const offset = { x: 10, y: -20 };
    const anchor = { x: 50, y: 30 };
    const next = zoomAround(offset, anchor, 1, 2);
    // Image point under the anchor: (anchor - offset) / scale.
    expect((anchor.x - next.x) / 2).toBeCloseTo((anchor.x - offset.x) / 1);
    expect((anchor.y - next.y) / 2).toBeCloseTo((anchor.y - offset.y) / 1);
  });

  it("zooming around the centre with no offset stays centred", () => {
    expect(zoomAround({ x: 0, y: 0 }, { x: 0, y: 0 }, 1, 3)).toEqual({ x: 0, y: 0 });
  });
});

describe("clampOffset", () => {
  it("centres an axis that fits inside the viewport", () => {
    expect(clampOffset({ x: 40, y: -40 }, { width: 100, height: 100 }, { width: 500, height: 500 }, 1)).toEqual({ x: 0, y: 0 });
  });

  it("limits panning to the overflowing extent", () => {
    expect(clampOffset({ x: 900, y: -900 }, { width: 1000, height: 600 }, { width: 500, height: 500 }, 1)).toEqual({ x: 250, y: -50 });
  });

  it("uses the rotated footprint", () => {
    expect(clampOffset({ x: 900, y: 900 }, { width: 1000, height: 600 }, { width: 500, height: 500 }, 1, 90)).toEqual({ x: 50, y: 250 });
  });
});
