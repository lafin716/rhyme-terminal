export type SessionMenuItem =
  | { id: string; label: string; kind: "page" | "browser" | "terminal" }
  | { id: string; label: string; kind: "agent"; agent: "claude" | "codex" };

/**
 * Rows of the session tab options menu. Terminals are one "New Terminal" row:
 * clicking it launches the default terminal, while its arrow opens a submenu
 * listing the shells available on this host (see `PaneTabs.vue`).
 */
export const SESSION_MENU_ITEMS: readonly SessionMenuItem[] = [
  { id: "page", label: "New File", kind: "page" },
  { id: "browser", label: "Open new browser", kind: "browser" },
  { id: "claude", label: "Claude", kind: "agent", agent: "claude" },
  { id: "codex", label: "Codex", kind: "agent", agent: "codex" },
  { id: "terminal", label: "New Terminal", kind: "terminal" },
];

/**
 * Section a row belongs to. The options menu draws a separator wherever two
 * neighbouring rows disagree, so page/browser stay one block however the user
 * reorders them.
 */
export function sessionMenuGroup(item: SessionMenuItem): "resource" | "agent" | "terminal" {
  return item.kind === "agent" ? "agent" : item.kind === "terminal" ? "terminal" : "resource";
}

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
