import { describe, expect, it } from "vitest";
import { initialPairingIp, parsePairingPreferences, serializePairingPreferences, validPort } from "./mobile-pairing";
const interfaces = [{ name: "LAN", ip: "192.168.1.2", isTailscale: false }, { name: "Tailscale", ip: "100.100.1.2", isTailscale: true }];
describe("mobile pairing preferences", () => {
  it("rejects corrupt, wrong-version and invalid port records", () => {
    for (const raw of [null, "{", "null", '[]', '{"version":2,"ip":"100.1.1.1","port":43123}', '{"version":1,"ip":"evil/host","port":43123}', ...[0, -1, 65536, 2.5, "43123", ""].map(port => JSON.stringify({ version: 1, ip: "100.1.1.1", port }))]) expect(parsePairingPreferences(raw)).toBeNull();
    expect(validPort(NaN)).toBe(false);
    for (const ip of ["999.1.1.1", "1.2.3", "::::", "abc"]) expect(parsePairingPreferences(JSON.stringify({ version: 1, ip, port: 43123 }))).toBeNull();
  });
  it("persists only nonsecret selection even if runtime data has extra fields", () => {
    const preferences = { ip: "fd7a:115c:a1e0::1", port: 43123, token: "secret", running: true };
    expect(JSON.parse(serializePairingPreferences(preferences))).toEqual({ version: 1, ip: "fd7a:115c:a1e0::1", port: 43123 });
    expect(parsePairingPreferences(serializePairingPreferences(preferences))).toEqual({ ip: preferences.ip, port: 43123 });
  });
  it("prefers Tailscale only for first selection and never replaces a missing remembered IP", () => {
    expect(initialPairingIp(interfaces, null)).toBe("100.100.1.2");
    expect(initialPairingIp(interfaces, "192.168.1.2")).toBe("192.168.1.2");
    expect(initialPairingIp(interfaces, "100.99.1.1")).toBe("100.99.1.1");
    expect(initialPairingIp([], null)).toBe("");
  });
});
