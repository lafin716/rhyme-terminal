import { describe, expect, it } from "vitest";
import { availableTerminalPresets, TERMINAL_PRESETS } from "./terminal-config";

const ids = (mac: boolean, selected?: Parameters<typeof availableTerminalPresets>[0]) =>
  availableTerminalPresets(selected, mac).map((preset) => preset.id);

describe("terminal presets per host", () => {
  it("hides the macOS shell on Windows and the Windows shells on macOS", () => {
    expect(ids(false)).toEqual(["windows-powershell", "powershell", "cmd", "wsl", "git-bash", "custom"]);
    expect(ids(true)).toEqual(["zsh", "custom"]);
  });

  it("keeps a configured preset listed even when it belongs to the other host", () => {
    expect(ids(false, "zsh")).toContain("zsh");
    expect(ids(true, "wsl")).toContain("wsl");
    expect(ids(true, "wsl")).not.toContain("cmd");
  });

  it("gives every preset a host so none can be silently dropped", () => {
    for (const preset of TERMINAL_PRESETS) {
      expect(["windows", "mac", "any"]).toContain(preset.platform);
    }
  });
});
