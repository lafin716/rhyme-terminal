import { describe, expect, it } from "vitest";
import { moveSessionMenuItem, normalizeSessionMenuOrder, orderedSessionMenuItems } from "./session-menu";

describe("session menu ordering", () => {
  it("keeps the original order for old or malformed preferences", () => {
    for (const value of [undefined, null, {}, "codex"]) {
      expect(normalizeSessionMenuOrder(value).slice(0, 4)).toEqual(["page", "claude", "codex", "default"]);
    }
  });

  it("retains saved order, removes invalid duplicates, and appends missing items", () => {
    const order = normalizeSessionMenuOrder(["terminal:wsl", "codex", "codex", "removed", 1]);
    expect(order.slice(0, 4)).toEqual(["terminal:wsl", "codex", "page", "claude"]);
    expect(new Set(order).size).toBe(order.length);
    expect(order).toHaveLength(normalizeSessionMenuOrder(undefined).length);
    expect(orderedSessionMenuItems(order).slice(0, 2).map((item) => item.label)).toEqual(["WSL", "Codex"]);
  });

  it("moves individual items across groups without mutating the saved array", () => {
    const original = normalizeSessionMenuOrder(undefined);
    const moved = moveSessionMenuItem(original, "claude", -1);
    expect(moved.slice(0, 3)).toEqual(["claude", "page", "codex"]);
    expect(original[0]).toBe("page");
    expect(moveSessionMenuItem(moved, "claude", 1)).toEqual(original);
    expect(moveSessionMenuItem(original, original[0], -1)).toEqual(original);
    expect(moveSessionMenuItem(original, original[original.length - 1], 1)).toEqual(original);
    expect(moveSessionMenuItem(original, "missing", 1)).toEqual(original);
  });
});
