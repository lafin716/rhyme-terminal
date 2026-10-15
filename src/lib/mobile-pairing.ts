import { invoke } from "@tauri-apps/api/core";

export interface NetworkInterface { name: string; ip: string; isTailscale: boolean }
export interface PairingStatus {
  running: boolean; ip: string | null; port: number | null; url: string | null;
  invite: { url: string; qrSvg: string; expiresAt: number } | null;
  pending: { requestId: string; name: string; verification: string }[];
  devices: { deviceId: string; name: string; connected: boolean }[];
}
export interface PairingPreferences { ip: string; port: number }
export const PAIRING_KEY = "winmux:mobile-pairing:v1";
export const emptyStatus = (): PairingStatus => ({ running: false, ip: null, port: null, url: null, invite: null, pending: [], devices: [] });
export function validPort(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 65535;
}
function isIpAddress(value: string): boolean {
  if (value.includes(":")) {
    try { return /^[0-9a-fA-F:.]+$/.test(value) && new URL(`http://[${value}]/`).hostname.startsWith("["); }
    catch { return false; }
  }
  const parts = value.split(".");
  return parts.length === 4 && parts.every(part => /^\d{1,3}$/.test(part) && Number(part) <= 255);
}
export function parsePairingPreferences(raw: string | null): PairingPreferences | null {
  try {
    const value: unknown = JSON.parse(raw ?? "null");
    if (!value || typeof value !== "object") return null;
    const record = value as Record<string, unknown>;
    if (record.version !== 1 || typeof record.ip !== "string" || record.ip.length > 64 || !isIpAddress(record.ip) || !validPort(record.port)) return null;
    return { ip: record.ip, port: record.port };
  } catch { return null; }
}
export function serializePairingPreferences(value: PairingPreferences): string {
  return JSON.stringify({ version: 1, ip: value.ip, port: value.port });
}
/** A missing remembered address must never silently select a different interface. */
export function initialPairingIp(interfaces: NetworkInterface[], remembered: string | null): string {
  return remembered ?? interfaces.find(item => item.isTailscale)?.ip ?? interfaces[0]?.ip ?? "";
}
export const mobilePairingApi = {
  interfaces: () => invoke<NetworkInterface[]>("mobile_pairing_interfaces"),
  status: () => invoke<PairingStatus>("mobile_pairing_status"),
  start: (ip: string, port: number) => invoke<PairingStatus>("mobile_pairing_start", { ip, port }),
  stop: () => invoke<PairingStatus>("mobile_pairing_stop"),
  invite: () => invoke<PairingStatus>("mobile_pairing_invite"),
  approve: (requestId: string, approve: boolean) => invoke<PairingStatus>("mobile_pairing_approve", { requestId, approve }),
  revoke: (deviceId: string) => invoke<PairingStatus>("mobile_pairing_revoke", { deviceId }),
};
