import { usePrefs, persistPrefs } from "./usePrefs";
import {
  setPanelWidth,
  togglePanelOpen,
  type PanelSide,
} from "../lib/shell-panels";

/**
 * Reactive UI layer over the Shell's panel state. The state lives in
 * `prefs.panels` (persisted in the `winmux:prefs:v1` envelope); this composable
 * exposes it plus the toggle/resize actions, persisting on each change.
 *
 * The pure state logic lives in `../lib/shell-panels.ts` and is unit-tested
 * there; this wrapper only adds Vue reactivity and persistence.
 */
export function useShellPanels() {
  const { prefs } = usePrefs();
  const panels = prefs.panels;

  function toggleLeft(): void {
    togglePanelOpen(panels, "left");
    persistPrefs();
  }

  function toggleRight(): void {
    togglePanelOpen(panels, "right");
    persistPrefs();
  }

  /** Live width update during a splitter drag; does not persist. */
  function resize(side: PanelSide, width: number): void {
    setPanelWidth(panels, side, width);
  }

  /** Persist the current panel state (e.g. at the end of a drag). */
  function commit(): void {
    persistPrefs();
  }

  return { panels, toggleLeft, toggleRight, resize, commit };
}
