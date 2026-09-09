import { afterEach, expect, it, vi } from "vitest";
import { loadPrefsFromStorage, usePrefs } from "./usePrefs";
import { moveSessionMenuItem, normalizeSessionMenuOrder } from "../lib/session-menu";

afterEach(() => vi.unstubAllGlobals());

it("persists and reloads session menu order while preserving other preferences", () => {
  const storage = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
  });
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
