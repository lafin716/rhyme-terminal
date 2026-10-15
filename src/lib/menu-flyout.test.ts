import { describe, expect, it } from "vitest";
import { toggleFlyoutGroup } from "./menu-flyout";

describe("toggleFlyoutGroup", () => {
  it("opens the selected first-depth menu and closes it when selected again", () => {
    expect(toggleFlyoutGroup(null, "file")).toBe("file");
    expect(toggleFlyoutGroup("file", "file")).toBeNull();
  });

  it("switches the visible submenu when another first-depth menu is selected", () => {
    expect(toggleFlyoutGroup("file", "terminal")).toBe("terminal");
  });
});
