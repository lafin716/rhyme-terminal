import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), mounted: null as null | (() => Promise<void>), unmounted: null as null | (() => void) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../composables/useI18n", () => ({ t: (message: string) => message }));
vi.mock("vue", async () => ({ ...await vi.importActual<typeof import("vue")>("vue"), onMounted: (cb: () => Promise<void>) => { mocks.mounted = cb; }, onUnmounted: (cb: () => void) => { mocks.unmounted = cb; } }));
const stopped = { running: false, ip: null, port: null, url: null, invite: null, pending: [], devices: [] };
const list = [{ name: "Tailscale", ip: "100.1.1.1", isTailscale: true }];
beforeEach(() => { vi.useFakeTimers(); mocks.invoke.mockReset(); mocks.mounted = null; mocks.unmounted = null; vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() }); });
afterEach(() => { mocks.unmounted?.(); vi.useRealTimers(); vi.unstubAllGlobals(); });
describe("mobile settings request lifecycle", () => {
  it("does not overlap polls or mutations and stops polling after unmount", async () => {
    mocks.invoke.mockImplementation(command => Promise.resolve(command === "mobile_pairing_interfaces" ? list : stopped));
    const { useMobilePairing } = await import("../composables/useMobilePairing");
    const state = useMobilePairing(); await mocks.mounted!();
    let resolve!: (value: typeof stopped) => void;
    mocks.invoke.mockImplementation(() => new Promise(done => { resolve = done; }));
    const before = mocks.invoke.mock.calls.length;
    await vi.advanceTimersByTimeAsync(1000);
    expect(state.busy.value).toBe(true);
    await vi.advanceTimersByTimeAsync(4000); await state.stop();
    expect(mocks.invoke.mock.calls.length).toBe(before + 1);
    resolve(stopped); await Promise.resolve(); await Promise.resolve();
    mocks.unmounted!(); await vi.advanceTimersByTimeAsync(10000);
    expect(mocks.invoke.mock.calls.length).toBe(before + 1);
  });
  it("ignores initial responses received after unmount", async () => {
    let release!: (value: unknown) => void;
    mocks.invoke.mockImplementation(command => command === "mobile_pairing_interfaces" ? Promise.resolve(list) : new Promise(done => { release = done; }));
    const { useMobilePairing } = await import("../composables/useMobilePairing");
    const state = useMobilePairing(); const mounting = mocks.mounted!();
    mocks.unmounted!();
    release({ ...stopped, running: true, ip: "100.1.1.1", port: 43123 }); await mounting;
    expect(state.status.value.running).toBe(false); expect(state.ready.value).toBe(false);
    await vi.advanceTimersByTimeAsync(10000); expect(mocks.invoke).toHaveBeenCalledTimes(2);
  });
  it("starts explicitly, persists no credentials, and uses camelCase approval arguments", async () => {
    const running = { ...stopped, running: true, ip: "100.1.1.1", port: 43123, url: "http://100.1.1.1:43123/" };
    mocks.invoke.mockImplementation(command => Promise.resolve(command === "mobile_pairing_interfaces" ? list : command === "mobile_pairing_status" ? stopped : running));
    const { useMobilePairing } = await import("../composables/useMobilePairing");
    const state = useMobilePairing(); await mocks.mounted!();
    expect(mocks.invoke).not.toHaveBeenCalledWith("mobile_pairing_start", expect.anything());
    await state.start();
    expect(mocks.invoke).toHaveBeenCalledWith("mobile_pairing_start", { ip: "100.1.1.1", port: 43123 });
    expect(localStorage.setItem).toHaveBeenCalledWith("winmux:mobile-pairing:v1", '{"version":1,"ip":"100.1.1.1","port":43123}');
    await state.approve("request", true);
    expect(mocks.invoke).toHaveBeenCalledWith("mobile_pairing_approve", { requestId: "request", approve: true });
  });
});
