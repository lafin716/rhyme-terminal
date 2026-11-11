// Pure helpers for the in-app update flow (see composables/useUpdater.ts).

export interface UpdateInfo {
  currentVersion: string;
  version: string;
  notes?: string | null;
  date?: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total?: number | null;
}

export type UpdateStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "installing"
  | "error";

/** First automatic check runs shortly after launch, then every six hours. */
export const UPDATE_FIRST_CHECK_MS = 15_000;
export const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

/** Whole download percentage, or null while the size is unknown. */
export function progressPercent(progress: UpdateProgress | null): number | null {
  if (!progress?.total || progress.total <= 0) return null;
  return Math.min(100, Math.max(0, Math.floor((progress.downloaded / progress.total) * 100)));
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++; }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}

/** True while a check or install is running and must not be started again. */
export function isUpdateBusy(status: UpdateStatus): boolean {
  return status === "checking" || status === "downloading" || status === "installing";
}
