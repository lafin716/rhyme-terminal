import { effectScope } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useTerminalZoom } from "./useTerminalZoom";

const scopes: ReturnType<typeof effectScope>[] = [];
function setup() {
  const scope = effectScope();
  scopes.push(scope);
  const apply = vi.fn();
  return { ...scope.run(() => useTerminalZoom(apply))!, apply, scope };
}

function key(overrides: Partial<KeyboardEvent> = {}) {
  return {
    ctrlKey: true, shiftKey: true, altKey: false, metaKey: false,
    key: "+", code: "Equal", isComposing: false,
    preventDefault: vi.fn(), stopPropagation: vi.fn(), ...overrides,
  } as unknown as KeyboardEvent;
}

function wheel(overrides: Partial<WheelEvent> = {}) {
  return { ...key(), deltaY: -120, ...overrides } as unknown as WheelEvent;
}

afterEach(() => {
  scopes.splice(0).forEach((scope) => scope.stop());
  vi.useRealTimers();
});

describe("terminal zoom", () => {
  it.each([
    ["+", "Equal", 110], ["_", "Minus", 90], ["-", "Minus", 90],
    ["+", "NumpadAdd", 110], ["-", "NumpadSubtract", 90],
  ])("handles %s (%s) without forwarding it to the terminal", (value, code, percent) => {
    const zoom = setup();
    const ev = key({ key: value, code });
    zoom.onZoomKeyDown(ev);
    expect(zoom.toastPercent.value).toBe(percent);
    expect(zoom.apply).toHaveBeenCalledWith(13 * percent / 100);
    expect(ev.preventDefault).toHaveBeenCalledOnce();
    expect(ev.stopPropagation).toHaveBeenCalledOnce();
  });

  it.each([
    { ctrlKey: false }, { shiftKey: false }, { altKey: true },
    { metaKey: true }, { isComposing: true }, { key: "c", code: "KeyC" },
  ])("preserves unrelated keys and IME input: %j", (overrides) => {
    const zoom = setup();
    const ev = key(overrides);
    zoom.onZoomKeyDown(ev);
    expect(zoom.apply).not.toHaveBeenCalled();
    expect(ev.preventDefault).not.toHaveBeenCalled();
  });

  it("zooms in both wheel directions, leaving ordinary scrolling intact", () => {
    const zoom = setup();
    const up = wheel();
    zoom.onZoomWheel(up);
    expect(zoom.toastPercent.value).toBe(110);
    expect(up.preventDefault).toHaveBeenCalledOnce();
    expect(up.stopPropagation).toHaveBeenCalledOnce();
    zoom.onZoomWheel(wheel({ deltaY: 120 }));
    expect(zoom.toastPercent.value).toBe(100);
    const ordinary = wheel({ shiftKey: false });
    zoom.onZoomWheel(ordinary);
    expect(ordinary.preventDefault).not.toHaveBeenCalled();
    zoom.onZoomWheel(wheel({ deltaY: 0 }));
    expect(zoom.apply).toHaveBeenCalledTimes(2);
  });

  it("clamps to 50–300% and still consumes gestures at the boundary", () => {
    const zoom = setup();
    for (let i = 0; i < 30; i++) zoom.onZoomWheel(wheel());
    expect(zoom.toastPercent.value).toBe(300);
    for (let i = 0; i < 30; i++) zoom.onZoomWheel(wheel({ deltaY: 120 }));
    expect(zoom.toastPercent.value).toBe(50);
    const boundary = wheel({ deltaY: 120 });
    zoom.onZoomWheel(boundary);
    expect(boundary.preventDefault).toHaveBeenCalledOnce();
    expect(zoom.apply).toHaveBeenLastCalledWith(6.5);
  });

  it("restarts the toast timeout for repeated input and clears it on disposal", () => {
    vi.useFakeTimers();
    const zoom = setup();
    zoom.onZoomKeyDown(key());
    vi.advanceTimersByTime(1000);
    zoom.onZoomKeyDown(key());
    vi.advanceTimersByTime(1000);
    expect(zoom.toastPercent.value).toBe(120);
    vi.advanceTimersByTime(100);
    expect(zoom.toastPercent.value).toBeNull();
    zoom.onZoomKeyDown(key());
    zoom.scope.stop();
    expect(zoom.toastPercent.value).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps pane zoom independent and lets only the latest pane show a toast", () => {
    vi.useFakeTimers();
    const first = setup();
    const second = setup();
    first.onZoomKeyDown(key());
    vi.advanceTimersByTime(600);
    second.onZoomWheel(wheel({ deltaY: 120 }));
    expect(first.toastPercent.value).toBeNull();
    expect(second.toastPercent.value).toBe(90);
    vi.advanceTimersByTime(500);
    first.scope.stop();
    expect(second.toastPercent.value).toBe(90);
    expect(first.apply).toHaveBeenLastCalledWith(14.3);
    expect(second.apply).toHaveBeenLastCalledWith(11.7);
    vi.advanceTimersByTime(600);
    expect(second.toastPercent.value).toBeNull();
  });
});
