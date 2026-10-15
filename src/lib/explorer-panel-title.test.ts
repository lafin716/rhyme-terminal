import { describe, expect, it } from "vitest";
import { explorerPanelTitle } from "./explorer-panel-title";

describe("explorerPanelTitle", () => {
  it("uses the current root folder name as the explorer title", () => {
    expect(explorerPanelTitle("winmux")).toBe("winmux");
  });

  it("uses Explorer until a root folder is known", () => {
    expect(explorerPanelTitle("")).toBe("Explorer");
  });
});
