import { reactive } from "vue";
import { DEFAULT_LOCALE, normalizeLocale } from "../lib/i18n";
import {
  loadPrefs,
  savePrefs,
  type PaletteUiMode,
  type Prefs,
} from "../lib/persistence";
import {
  createDefaultPanelsState,
  panelsFromStored,
  serializePanels,
} from "../lib/shell-panels";
import {
  defaultTerminalConfig,
  normalizeTerminalConfig,
} from "../lib/terminal-config";

const prefs = reactive<Prefs>({
  language: DEFAULT_LOCALE,
  showAccountProfile: true,
  skipKillSessionConfirm: false,
  defaultTerminal: defaultTerminalConfig(),
  paletteUiMode: "context",
  panels: createDefaultPanelsState(),
  defaultProfileId: { claude: null, codex: null },
});

export function loadPrefsFromStorage(): void {
  const stored = loadPrefs();
  if (!stored) return;
  const paletteUiMode: PaletteUiMode = stored.paletteUiMode === "radial"
    ? "radial"
    : "context";
  // Resolve panels from the stored payload, migrating the legacy `sidebarMode`
  // preference when no `panels` payload is present.
  const migratedFromLegacy = stored.panels === undefined && "sidebarMode" in stored;
  const panels = panelsFromStored(stored);
  Object.assign(prefs, {
    ...stored,
    language: normalizeLocale(stored.language),
    showAccountProfile: typeof stored.showAccountProfile === "boolean" ? stored.showAccountProfile : true,
    defaultTerminal: normalizeTerminalConfig(stored.defaultTerminal),
    paletteUiMode,
    panels,
  });
  // The legacy `sidebarMode` field is superseded by `panels`; don't keep it around.
  if ("sidebarMode" in prefs) {
    delete (prefs as Record<string, unknown>).sidebarMode;
  }
  // Persist once after a legacy migration so the old field is dropped from
  // storage and subsequent launches read `panels` directly.
  if (migratedFromLegacy) persistPrefs();
}

/** Persist the current prefs, snapshotting the reactive panel state to plain data. */
export function persistPrefs(): void {
  savePrefs({ ...prefs, panels: serializePanels(prefs.panels) });
}

export function setPref<K extends keyof Prefs>(key: K, value: Prefs[K]): void {
  prefs[key] = value;
  persistPrefs();
}

export function usePrefs() {
  return { prefs, setPref };
}
