import { afterEach, expect, it, vi } from "vitest";
import { moveSessionMenuItem, normalizeSessionMenuOrder } from "../lib/session-menu";

afterEach(() => {
  vi.resetModules();
  vi.unstubAllGlobals();
});

function stubBrowser(platform: string) {
  const storage = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
  });
  vi.stubGlobal("navigator", { platform });
  return storage;
}

it("persists and reloads session menu order while preserving Windows terminal preferences", async () => {
  const storage = stubBrowser("Win32");
  const { loadPrefsFromStorage, usePrefs } = await import("./usePrefs");
  const { prefs, setPref } = usePrefs();
  const order = moveSessionMenuItem(undefined, "codex", -1);
  setPref("sessionMenuOrder", order);
  prefs.sessionMenuOrder = [];
  loadPrefsFromStorage();
  expect(prefs.sessionMenuOrder).toEqual(order);
  expect(prefs.defaultTerminal.preset).toBe("windows-powershell");
  const saved = JSON.parse(storage.get("winmux:prefs:v1")!);
  expect(saved.version).toBe(1);
  saved.prefs.sessionMenuOrder = ["codex", "codex", "unknown"];
  storage.set("winmux:prefs:v1", JSON.stringify(saved));
  loadPrefsFromStorage();
  expect(prefs.sessionMenuOrder).toEqual(normalizeSessionMenuOrder(["codex"]));
  setPref("sessionMenuOrder", normalizeSessionMenuOrder(undefined));
  loadPrefsFromStorage();
  expect(prefs.sessionMenuOrder?.[0]).toBe("page");
});

it("preserves the macOS default terminal while reloading session menu order", async () => {
  const storage = stubBrowser("MacIntel");
  const { loadPrefsFromStorage, usePrefs } = await import("./usePrefs");
  const { prefs, setPref } = usePrefs();
  const order = moveSessionMenuItem(undefined, "codex", -1);
  setPref("sessionMenuOrder", order);
  prefs.sessionMenuOrder = [];
  loadPrefsFromStorage();
  expect(prefs.sessionMenuOrder).toEqual(order);
  expect(prefs.defaultTerminal).toEqual({
    preset: "zsh",
    program: "/bin/zsh",
    args: ["-l", "-i"],
  });
  expect(JSON.parse(storage.get("winmux:prefs:v1")!).prefs.defaultTerminal.preset).toBe("zsh");
});
