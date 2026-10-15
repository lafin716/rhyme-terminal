import { nextTick } from "vue";
import { afterEach, expect, it, vi } from "vitest";

afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it("persists, reloads, clears and resets direct, wheel and prefix overrides", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("window", globalThis);
  const storage = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
  });
  const { useKeybindings, loadKeybindingsFromStorage } = await import("./useKeybindings");
  const kb = useKeybindings();
  kb.setBinding("terminal.zoomIn", { key: "F8" });
  kb.setBinding("terminal.zoomInWheel", { key: "WheelUp", alt: true });
  kb.setBinding("prefix.activate", { key: "a", ctrl: true });
  kb.setPrefix("session.new", "x");
  kb.setPrefix("pane.selectTab0", null);
  await nextTick();
  vi.advanceTimersByTime(150);
  const saved = JSON.parse(storage.get("winmux:keybindings:v1")!);
  expect(saved.version).toBe(1);
  expect(saved.bindings["terminal.zoomIn"]).toEqual({ key: "F8" });
  kb.state.overrides = {};
  loadKeybindingsFromStorage();
  expect(kb.bindingFor("terminal.zoomInWheel")).toEqual({ key: "WheelUp", alt: true });
  expect(kb.prefixFor("session.new")).toBe("x");
  expect(kb.prefixFor("pane.selectTab0")).toBeNull();
  kb.setBinding("terminal.zoomIn", null);
  expect(kb.bindingFor("terminal.zoomIn")).toBeNull();
  kb.resetBinding("terminal.zoomIn");
  expect(kb.bindingFor("terminal.zoomIn")?.key).toBe("+");
  kb.resetPrefix("session.new");
  expect(kb.prefixFor("session.new")).toBe("c");
  kb.resetAll();
  expect(kb.bindingFor("prefix.activate")).toEqual({ ctrl: true, key: "b" });
  expect(kb.prefixFor("pane.selectTab0")).toBe("0");
  await nextTick();
  vi.runOnlyPendingTimers();
});
