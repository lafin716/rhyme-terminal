import { computed } from "vue";
import { usePrefs } from "./usePrefs";
import { translate } from "../lib/i18n";

export function t(message: string, params?: Record<string, string | number>): string {
  return translate(usePrefs().prefs.language, message, params);
}

export function useI18n() {
  const { prefs } = usePrefs();
  return { t, locale: computed(() => prefs.language) };
}
