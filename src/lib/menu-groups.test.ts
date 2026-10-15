import { describe, expect, it } from "vitest";
import { groupMenuItems } from "./menu-groups";

describe("groupMenuItems", () => {
  it("preserves each top-level menu as a labelled dropdown section", () => {
    const groups = groupMenuItems([
      { id: "file", label: "File", items: ["New Terminal", "Quit"] },
      { id: "terminal", label: "Terminal", items: ["Split Horizontally"] },
    ]);

    expect(groups).toEqual([
      { id: "file", label: "File", items: ["New Terminal", "Quit"] },
      { id: "terminal", label: "Terminal", items: ["Split Horizontally"] },
    ]);
  });
});
