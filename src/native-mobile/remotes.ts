import { gatewayInvitation } from "./invitation";

/**
 * Saved remote PCs. Only the gateway origin and a label are stored here:
 * invites are one-time and device tokens stay in the PC origin's sessionStorage.
 */
export interface RemoteEntry {
  /** Normalised gateway origin, e.g. `http://100.70.1.2:43123`. Also the identity. */
  origin: string;
  name: string;
  lastConnectedAt: number | null;
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const REMOTES_KEY = "winmux:mobile-remotes:v1";
export const MAX_REMOTES = 50;
export const MAX_NAME_LENGTH = 40;

export function normalizeOrigin(raw: string): string {
  const url = new URL(raw.trim());
  if (url.protocol !== "http:" && url.protocol !== "https:") throw new Error("HTTP 또는 HTTPS 주소만 사용할 수 있습니다.");
  if (url.username || url.password) throw new Error("계정 정보가 포함된 주소는 사용할 수 없습니다.");
  return url.origin;
}

function cleanName(raw: string, fallback: string): string {
  const name = raw.replace(/\s+/g, " ").trim().slice(0, MAX_NAME_LENGTH);
  return name || fallback;
}

function isEntry(value: unknown): value is RemoteEntry {
  if (!value || typeof value !== "object") return false;
  const entry = value as Record<string, unknown>;
  if (typeof entry.origin !== "string" || typeof entry.name !== "string") return false;
  if (entry.lastConnectedAt !== null && typeof entry.lastConnectedAt !== "number") return false;
  try { return normalizeOrigin(entry.origin) === entry.origin; } catch { return false; }
}

/** Most recently connected first; never-connected entries last, by name. */
export function sortRemotes(remotes: readonly RemoteEntry[]): RemoteEntry[] {
  return [...remotes].sort((a, b) => (b.lastConnectedAt ?? -1) - (a.lastConnectedAt ?? -1) || a.name.localeCompare(b.name));
}

export function loadRemotes(storage: StorageLike): RemoteEntry[] {
  try {
    const parsed: unknown = JSON.parse(storage.getItem(REMOTES_KEY) ?? "null");
    if (!parsed || typeof parsed !== "object") return [];
    const { version, remotes } = parsed as { version?: unknown; remotes?: unknown };
    if (version !== 1 || !Array.isArray(remotes)) return [];
    const byOrigin = new Map<string, RemoteEntry>();
    for (const entry of remotes.filter(isEntry)) {
      if (byOrigin.has(entry.origin)) continue;
      const { origin, name, lastConnectedAt } = entry;
      byOrigin.set(origin, { origin, name: cleanName(name, new URL(origin).host), lastConnectedAt });
    }
    return sortRemotes([...byOrigin.values()]).slice(0, MAX_REMOTES);
  } catch { return []; }
}

export function saveRemotes(storage: StorageLike, remotes: readonly RemoteEntry[]): void {
  const plain = remotes.map(({ origin, name, lastConnectedAt }) => ({ origin, name, lastConnectedAt }));
  storage.setItem(REMOTES_KEY, JSON.stringify({ version: 1, remotes: plain }));
}

/** Validate an invite link, remember its gateway and return where to navigate. */
export function connectWithInvitation(remotes: readonly RemoteEntry[], raw: string, now: number): { remotes: RemoteEntry[]; target: string } {
  const target = gatewayInvitation(raw);
  const origin = normalizeOrigin(target);
  const existing = remotes.find(entry => entry.origin === origin);
  if (!existing && remotes.length >= MAX_REMOTES) throw new Error(`원격 연결은 최대 ${MAX_REMOTES}개까지 저장할 수 있습니다.`);
  const others = remotes.filter(entry => entry.origin !== origin);
  const entry: RemoteEntry = { origin, name: existing?.name ?? new URL(origin).host, lastConnectedAt: now };
  return { remotes: sortRemotes([...others, entry]), target };
}

/** Reconnect without an invite: the PC client reuses this app session's device token, if any. */
export function remoteTarget(origin: string): string {
  const url = new URL(normalizeOrigin(origin));
  url.hash = new URLSearchParams({ native: "android" }).toString();
  return url.toString();
}

export function touchRemote(remotes: readonly RemoteEntry[], origin: string, now: number): RemoteEntry[] {
  return sortRemotes(remotes.map(entry => entry.origin === origin ? { ...entry, lastConnectedAt: now } : entry));
}

export function renameRemote(remotes: readonly RemoteEntry[], origin: string, name: string): RemoteEntry[] {
  return sortRemotes(remotes.map(entry => entry.origin === origin ? { ...entry, name: cleanName(name, new URL(origin).host) } : entry));
}

export function removeRemote(remotes: readonly RemoteEntry[], origin: string): RemoteEntry[] {
  return remotes.filter(entry => entry.origin !== origin);
}
