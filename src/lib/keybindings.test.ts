import { describe, expect, it } from "vitest";
import { ACTIONS, captureKeybinding, captureWheelBinding, formatKeybinding, matchesEvent, matchesWheelEvent, sameBinding } from "./keybindings";

describe("configurable shortcuts", () => {
  it("uses the same shifted minus identity for capture, matching and conflicts", () => {
    const event = { key: "_", code: "Minus", ctrlKey: true, shiftKey: true, altKey: false, metaKey: false } as KeyboardEvent;
    const binding = captureKeybinding(event);
    expect(binding?.key).toBe("-");
    expect(matchesEvent(binding, event)).toBe(true);
    expect(sameBinding(binding, { key: "_", ctrl: true, shift: true })).toBe(true);
    expect(formatKeybinding(binding)).toBe("Ctrl+Shift+-");
  });
  it("captures and matches wheel direction and exact modifiers", () => {
    const event = { deltaY: -120, ctrlKey: true, shiftKey: false, altKey: true } as WheelEvent;
    const binding = captureWheelBinding(event);
    expect(binding?.key).toBe("WheelUp");
    expect(matchesWheelEvent(binding, event)).toBe(true);
    expect(matchesWheelEvent(binding, { ...event, deltaY: 120 })).toBe(false);
    expect(matchesWheelEvent(binding, { ...event, shiftKey: true })).toBe(false);
    expect(matchesWheelEvent(null, event)).toBe(false);
    expect(captureWheelBinding({ ...event, deltaY: 0 })).toBeNull();
  });
  it("formats the current prefix key instead of a hard-coded Ctrl+B", () => {
    expect(formatKeybinding(null, "c", { ctrl: true, key: "a" })).toBe("Ctrl+A c");
  });
  it("registers fixed app commands with unique IDs and appropriate scopes", () => {
    expect(new Set(ACTIONS.map(a => a.id)).size).toBe(ACTIONS.length);
    expect(ACTIONS.filter(a => a.id.startsWith("session.focusIndex"))).toHaveLength(15);
    expect(ACTIONS.filter(a => a.id.startsWith("pane.selectTab"))).toHaveLength(10);
    for (const id of ["terminal.zoomIn", "terminal.zoomOut", "terminal.zoomInWheel", "terminal.zoomOutWheel", "terminal.copy", "terminal.paste", "editor.save", "editor.find", "prefix.activate"]) {
      expect(ACTIONS.find(a => a.id === id)?.scope, id).toBeTruthy();
    }
  });
});
