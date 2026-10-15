import { ko } from "./locales/ko";

export type Locale = "ko" | "en";
export const DEFAULT_LOCALE: Locale = "ko";

export function normalizeLocale(value: unknown): Locale {
  return value === "en" ? "en" : DEFAULT_LOCALE;
}

// English source messages are stable keys and also the fallback for missing translations.
export function translate(locale: Locale, message: string, params: Record<string, string | number> = {}): string {
  const text = locale === "ko" ? ko[message] ?? message : message;
  return text.replace(/\{(\w+)\}/g, (placeholder, key: string) =>
    Object.prototype.hasOwnProperty.call(params, key) ? String(params[key]) : placeholder);
}
