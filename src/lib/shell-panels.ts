// Headless panel-state for the Shell's left Navigator and right Explorer.
//
// This module owns the non-visual panel state only — open flags and pixel
// widths — plus the pure operations over it (toggle, clamp, serialize,
// migrate). It has no Vue or DOM dependency so it can be unit-tested directly.

export type PanelSide = "left" | "right";

export interface PanelState {
  /** Whether the panel is shown at all (false = fully hidden). */
  open: boolean;
  /** Pixel width when open. */
  width: number;
}

export interface ShellPanelsState {
  left: PanelState;
  right: PanelState;
}

export interface PanelLimits {
  min: number;
  max: number;
  default: number;
}

export const PANEL_LIMITS: Record<PanelSide, PanelLimits> = {
  left: { min: 160, max: 480, default: 220 },
  right: { min: 180, max: 560, default: 300 },
};

export function createDefaultPanelsState(): ShellPanelsState {
  return {
    left: { open: true, width: PANEL_LIMITS.left.default },
    right: { open: false, width: PANEL_LIMITS.right.default },
  };
}

export function togglePanelOpen(state: ShellPanelsState, side: PanelSide): void {
  state[side].open = !state[side].open;
}

export function clampPanelWidth(side: PanelSide, width: number): number {
  const { min, max } = PANEL_LIMITS[side];
  if (!Number.isFinite(width)) return min;
  return Math.min(max, Math.max(min, Math.round(width)));
}

export function setPanelWidth(
  state: ShellPanelsState,
  side: PanelSide,
  width: number,
): void {
  state[side].width = clampPanelWidth(side, width);
}

/** Plain, deep-copied snapshot suitable for persistence. */
export function serializePanels(state: ShellPanelsState): ShellPanelsState {
  return {
    left: { ...state.left },
    right: { ...state.right },
  };
}

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : {};
}

function readPanel(
  side: PanelSide,
  raw: unknown,
  fallback: PanelState,
): PanelState {
  const src = asRecord(raw);
  return {
    open: typeof src.open === "boolean" ? src.open : fallback.open,
    width: typeof src.width === "number" ? clampPanelWidth(side, src.width) : fallback.width,
  };
}

/**
 * Rebuild panel state from persisted (possibly partial or foreign) data.
 * Missing fields fall back to defaults; unknown fields are dropped; widths are
 * clamped. Legacy collapsed flags are ignored so an open panel is always full.
 */
export function deserializePanels(data: unknown): ShellPanelsState {
  const defaults = createDefaultPanelsState();
  const raw = asRecord(data);
  return {
    left: readPanel("left", raw.left, defaults.left),
    right: readPanel("right", raw.right, defaults.right),
  };
}

/**
 * Translate the old 3-mode sidebar preference (compact | expanded | minimal)
 * into the current two-state panel model:
 *   expanded/compact → open (full), minimal → closed.
 * The right panel is new, so it starts at its default (hidden).
 */
export function migrateLegacySidebarMode(mode: unknown): ShellPanelsState {
  const state = createDefaultPanelsState();
  switch (mode) {
    case "expanded":
    case "compact":
      state.left.open = true;
      break;
    case "minimal":
      state.left.open = false;
      break;
    default:
      // Unknown or absent: keep defaults.
      break;
  }
  return state;
}

/** Shape of what may live in a persisted prefs payload for panel state. */
export interface StoredPanelSource {
  panels?: unknown;
  sidebarMode?: unknown;
}

/**
 * Resolve panel state from persisted prefs: prefer a `panels` payload, else
 * migrate the legacy `sidebarMode`, else fall back to defaults.
 */
export function panelsFromStored(stored: StoredPanelSource): ShellPanelsState {
  if (stored.panels !== undefined) return deserializePanels(stored.panels);
  return migrateLegacySidebarMode(stored.sidebarMode);
}
