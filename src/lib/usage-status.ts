import { DEFAULT_LOCALE, translate, type Locale } from "./i18n";

export interface UsageStat {
  /** 0-100, or `null` when no data source is connected yet. */
  percentUsed: number | null;
  /** Epoch ms when the limit is expected to reset, or `null` when unknown. */
  resetsAt: number | null;
}

export function formatUsagePercent(stat: UsageStat): string {
  if (stat.percentUsed === null) return "—%";
  const clamped = Math.min(100, Math.max(0, stat.percentUsed));
  return `${Math.round(clamped)}%`;
}

export function formatUsageReset(stat: UsageStat, now: number = Date.now(), locale: Locale = DEFAULT_LOCALE): string {
  if (stat.resetsAt === null) return translate(locale, "Not connected");
  const diffMs = stat.resetsAt - now;
  if (diffMs <= 0) return translate(locale, "Reset");
  const totalMinutes = Math.round(diffMs / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours > 0) return translate(locale, "Resets in {hours}h {minutes}m", { hours, minutes });
  return translate(locale, "Resets in {minutes}m", { minutes });
}
