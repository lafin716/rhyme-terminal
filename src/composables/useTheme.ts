import { computed, watchEffect, type WatchHandle } from "vue";
import { setPref, usePrefs } from "./usePrefs";
import {
  ACCENT_THEMES,
  accentThemeById,
  applyAccentTheme,
  type AccentThemeId,
} from "../lib/theme";

const { prefs } = usePrefs();

const activeTheme = computed(() => accentThemeById(prefs.accentTheme));

/**
 * Mirrors the chosen accent onto the document root as CSS custom properties.
 * Started once from `main.ts`; every component then styles itself through
 * `var(--accent…)` and re-tints without re-rendering.
 */
export function startAccentThemeSync(root: HTMLElement = document.documentElement): WatchHandle {
  return watchEffect(() => applyAccentTheme(activeTheme.value, root));
}

export function setAccentTheme(id: AccentThemeId): void {
  setPref("accentTheme", id);
}

export function useTheme() {
  return { themes: ACCENT_THEMES, activeTheme, setAccentTheme };
}
