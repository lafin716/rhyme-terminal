import { describe, expect, it } from "vitest";
import { availableRightPanelTabs, resolveRightPanelTab } from "./right-panel-tabs";

describe("right panel tabs", () => {
  it("opens the Files tab by default", () => {
    expect(resolveRightPanelTab()).toBe("files");
  });

  it("keeps a requested tab only when it is available", () => {
    expect(resolveRightPanelTab("files")).toBe("files");
    expect(resolveRightPanelTab("git")).toBe("files");
  });

  it("exposes only Files until its companion panels are implemented", () => {
    expect(availableRightPanelTabs).toEqual(["files"]);
  });
});
