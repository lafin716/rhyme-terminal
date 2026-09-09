export type Keybinding = {
  key?: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  meta?: boolean;
  prefix?: string;
} | null;

export interface ActionDef {
  id: string;
  label: string;
  category: "session" | "pane" | "window" | "settings" | "terminal" | "editor";
  scope?: "terminal" | "editor" | "prefix";
  default: Keybinding;
  defaultPrefix: string | null;
}

export const ACTIONS: ReadonlyArray<ActionDef> = [
  { id: "terminal.zoomIn", label: "Zoom In Terminal", category: "terminal", scope: "terminal", default: { ctrl: true, shift: true, key: "+" }, defaultPrefix: null },
  { id: "terminal.zoomOut", label: "Zoom Out Terminal", category: "terminal", scope: "terminal", default: { ctrl: true, shift: true, key: "-" }, defaultPrefix: null },
  { id: "terminal.zoomInWheel", label: "Zoom In Terminal (Wheel)", category: "terminal", scope: "terminal", default: { ctrl: true, shift: true, key: "WheelUp" }, defaultPrefix: null },
  { id: "terminal.zoomOutWheel", label: "Zoom Out Terminal (Wheel)", category: "terminal", scope: "terminal", default: { ctrl: true, shift: true, key: "WheelDown" }, defaultPrefix: null },
  { id: "terminal.copy", label: "Copy Terminal Selection", category: "terminal", scope: "terminal", default: { ctrl: true, shift: true, key: "c" }, defaultPrefix: null },
  { id: "terminal.copyOrInterrupt", label: "Copy Selection or Interrupt", category: "terminal", scope: "terminal", default: { ctrl: true, key: "c" }, defaultPrefix: null },
  { id: "terminal.paste", label: "Paste into Terminal", category: "terminal", scope: "terminal", default: { ctrl: true, shift: true, key: "v" }, defaultPrefix: null },
  { id: "terminal.pasteAlternate", label: "Paste into Terminal (Alternate)", category: "terminal", scope: "terminal", default: { ctrl: true, key: "v" }, defaultPrefix: null },
  { id: "editor.save", label: "Save File", category: "editor", scope: "editor", default: { ctrl: true, key: "s" }, defaultPrefix: null },
  { id: "editor.find", label: "Find in File", category: "editor", scope: "editor", default: { ctrl: true, key: "f" }, defaultPrefix: null },
  { id: "prefix.activate", label: "Activate Prefix Key", category: "settings", scope: "prefix", default: { ctrl: true, key: "b" }, defaultPrefix: null },
  ...Array.from({ length: 15 }, (_, i): ActionDef => ({
    id: `session.focusIndex${i + 1}`, label: `Focus Session ${i + 1}`, category: "session",
    default: { alt: true, key: i < 9 ? String(i + 1) : String.fromCharCode(97 + i - 9) }, defaultPrefix: null,
  })),
  ...Array.from({ length: 10 }, (_, i): ActionDef => ({
    id: `pane.selectTab${i}`, label: `Select Pane Tab ${i}`, category: "pane", default: null, defaultPrefix: String(i),
  })),
  { id: "session.new",          label: "New Terminal",         category: "session", default: { ctrl: true, key: "n" }, defaultPrefix: "c" },
  { id: "session.newClaude",    label: "Launch Claude",        category: "session", default: null,                     defaultPrefix: "C" },
  { id: "session.kill",         label: "Kill Focused Session", category: "session", default: null,                     defaultPrefix: "&" },
  { id: "session.killNoConfirm",label: "Close Focused Session",category: "session", default: { ctrl: true, key: "w" }, defaultPrefix: null },
  { id: "session.rename",       label: "Rename Session",       category: "session", default: null,                     defaultPrefix: "," },
  { id: "session.cycleNext",    label: "Next Tab in Pane",     category: "session", default: null,                     defaultPrefix: "n" },
  { id: "session.cyclePrev",    label: "Previous Tab in Pane", category: "session", default: null,                     defaultPrefix: "p" },
  { id: "pane.splitHorizontal", label: "Split Horizontally",   category: "pane",    default: null,                     defaultPrefix: "%" },
  { id: "pane.splitVertical",   label: "Split Vertically",     category: "pane",    default: null,                     defaultPrefix: "\"" },
  { id: "window.detach",        label: "Hide Window",          category: "window",  default: null,                     defaultPrefix: "d" },
  { id: "settings.open",        label: "Open Settings",        category: "settings",default: { ctrl: true, key: "," }, defaultPrefix: null },
  { id: "session.focusPrev",        label: "Focus Previous Session (Global)", category: "session", default: { ctrl: true, shift: true, key: "{" },           defaultPrefix: null },
  { id: "session.focusNext",        label: "Focus Next Session (Global)",     category: "session", default: { ctrl: true, shift: true, key: "}" },           defaultPrefix: null },
  { id: "pane.splitOrMoveLeft",     label: "Split or Move Left",              category: "pane",    default: { ctrl: true, alt: true, key: "ArrowLeft" },     defaultPrefix: null },
  { id: "pane.splitOrMoveRight",    label: "Split or Move Right",             category: "pane",    default: { ctrl: true, alt: true, key: "ArrowRight" },    defaultPrefix: null },
  { id: "pane.splitOrMoveUp",       label: "Split or Move Up",                category: "pane",    default: { ctrl: true, alt: true, key: "ArrowUp" },       defaultPrefix: null },
  { id: "pane.splitOrMoveDown",     label: "Split or Move Down",              category: "pane",    default: { ctrl: true, alt: true, key: "ArrowDown" },     defaultPrefix: null },
  { id: "pane.quadrantTopLeft",     label: "Quadrant Top-Left Split",         category: "pane",    default: { ctrl: true, alt: true, key: "i" },             defaultPrefix: null },
  { id: "pane.quadrantTopRight",    label: "Quadrant Top-Right Split",        category: "pane",    default: { ctrl: true, alt: true, key: "o" },             defaultPrefix: null },
  { id: "pane.quadrantBottomLeft",  label: "Quadrant Bottom-Left Split",      category: "pane",    default: { ctrl: true, alt: true, key: "k" },             defaultPrefix: null },
  { id: "pane.quadrantBottomRight", label: "Quadrant Bottom-Right Split",     category: "pane",    default: { ctrl: true, alt: true, key: "l" },             defaultPrefix: null },
  { id: "view.toggleLeftPanel",     label: "Toggle Left Panel (Navigator)",   category: "window",  default: { ctrl: true, shift: true, key: "b" },           defaultPrefix: null },
  { id: "view.toggleRightPanel",    label: "Toggle Right Panel (Explorer)",   category: "window",  default: { ctrl: true, shift: true, key: "e" },           defaultPrefix: null },
  { id: "view.quickOpen",           label: "Quick Open",                      category: "window",  default: { ctrl: true, key: "p" },                       defaultPrefix: null },
];

export type ActionId = (typeof ACTIONS)[number]["id"];

export function getAction(id: string): ActionDef | undefined {
  return ACTIONS.find((a) => a.id === id);
}

function normalizeKey(k: string): string {
  if (k.length === 1) return k.toLowerCase();
  return k;
}

export function formatKeybinding(b: Keybinding, prefix?: string | null, prefixKey: Keybinding = { ctrl: true, key: "b" }): string {
  const parts: string[] = [];
  if (b && b.key) {
    if (b.ctrl) parts.push("Ctrl");
    if (b.shift) parts.push("Shift");
    if (b.alt) parts.push("Alt");
    if (b.meta) parts.push("Meta");
    parts.push(b.key.length === 1 ? b.key.toUpperCase() : b.key);
  }
  let s = parts.join("+");
  if (prefix) {
    const pfx = `${formatKeybinding(prefixKey)} ${prefix}`;
    s = s ? `${s}  /  ${pfx}` : pfx;
  }
  return s || "(unbound)";
}

export function matchesEvent(b: Keybinding, ev: KeyboardEvent): boolean {
  if (!b || !b.key) return false;
  if (!!b.ctrl !== ev.ctrlKey) return false;
  if (!!b.shift !== ev.shiftKey) return false;
  if (!!b.alt !== ev.altKey) return false;
  if (!!b.meta !== ev.metaKey) return false;
  return normalizeKey(b.key) === normalizeKey(ev.key) ||
    (!!b.shift && b.key === "-" && ev.key === "_") ||
    (!!b.shift && b.key === "+" && ev.code === "Equal");
}

function bindingKey(b: NonNullable<Keybinding>): string {
  return b.shift && b.key === "_" ? "-" : normalizeKey(b.key ?? "");
}

export function captureWheelBinding(ev: WheelEvent): Keybinding {
  if (ev.deltaY === 0) return null;
  return { key: ev.deltaY < 0 ? "WheelUp" : "WheelDown", ctrl: ev.ctrlKey, shift: ev.shiftKey, alt: ev.altKey, meta: ev.metaKey };
}

export function matchesWheelEvent(b: Keybinding, ev: WheelEvent): boolean {
  const wheel = captureWheelBinding(ev);
  return !!wheel && sameBinding(b, wheel);
}

export function captureKeybinding(ev: KeyboardEvent): Keybinding {
  if (["Control", "Shift", "Alt", "Meta"].includes(ev.key)) return null;
  return {
    key: ev.shiftKey && ev.key === "_" ? "-" : ev.key,
    ctrl: ev.ctrlKey,
    shift: ev.shiftKey,
    alt: ev.altKey,
    meta: ev.metaKey,
  };
}

export function sameBinding(a: Keybinding, b: Keybinding): boolean {
  if (!a || !b) return a === b;
  if (!a.key || !b.key) return false;
  return (
    bindingKey(a) === bindingKey(b) &&
    !!a.ctrl === !!b.ctrl &&
    !!a.shift === !!b.shift &&
    !!a.alt === !!b.alt &&
    !!a.meta === !!b.meta
  );
}
