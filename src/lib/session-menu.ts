import { TERMINAL_PRESETS, type TerminalPreset } from "./terminal-config";

type SessionMenuItem =
  | { id: string; label: string; kind: "page" | "default" }
  | { id: string; label: string; kind: "agent"; agent: "claude" | "codex" }
  | { id: string; label: string; kind: "terminal"; preset: TerminalPreset };

export const SESSION_MENU_ITEMS: readonly SessionMenuItem[] = [
  { id: "page", label: "Open new page", kind: "page" },
  { id: "claude", label: "Claude", kind: "agent", agent: "claude" },
  { id: "codex", label: "Codex", kind: "agent", agent: "codex" },
  { id: "default", label: "Default terminal", kind: "default" },
  ...TERMINAL_PRESETS.filter((preset) => preset.id !== "custom").map((preset) => ({
    id: `terminal:${preset.id}`, label: preset.label, kind: "terminal" as const, preset: preset.id,
  })),
];

/** Keep saved positions, drop obsolete/duplicate IDs, and append newly added items. */
export function normalizeSessionMenuOrder(value: unknown): string[] {
  const defaults = SESSION_MENU_ITEMS.map((item) => item.id);
  const saved = Array.isArray(value)
    ? value.filter((id): id is string => typeof id === "string" && defaults.includes(id))
    : [];
  return [...new Set([...saved, ...defaults])];
}

export function orderedSessionMenuItems(value: unknown): SessionMenuItem[] {
  return normalizeSessionMenuOrder(value).map((id) => SESSION_MENU_ITEMS.find((item) => item.id === id)!);
}

export function moveSessionMenuItem(value: unknown, id: string, offset: -1 | 1): string[] {
  const order = normalizeSessionMenuOrder(value);
  const index = order.indexOf(id);
  const target = index + offset;
  if (index >= 0 && target >= 0 && target < order.length) {
    [order[index], order[target]] = [order[target], order[index]];
  }
  return order;
}
