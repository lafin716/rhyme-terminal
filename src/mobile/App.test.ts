import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope, nextTick, type EffectScope } from "vue";
import type { MobileClientOptions } from "./client";
import type { SessionInfo } from "./types";
import MobileApp from "./App.vue";

const mocks = vi.hoisted(() => ({
  options: null as MobileClientOptions | null,
  mounted: null as (() => void) | null,
  unmounted: null as (() => void) | null,
  terminals: [] as { options: { disableStdin: boolean }; input: ((data: string) => void) | null }[],
  sendInput: vi.fn(),
}));
const info: SessionInfo = { id: "one", name: "w1.shell", shell: "pwsh", cwd: null, cols: 80, rows: 24 };
vi.mock("./client", () => ({ MobileClient: class {
  constructor(options: MobileClientOptions) { mocks.options = options; }
  connect() { mocks.options!.onState({ phase: "authenticated", verification: null, error: null, sessions: [info], attached: null }); }
  disconnect() {}
  async attach() {
    await mocks.options!.onAttach({ info, scrollback: "" }, () => true);
    mocks.options!.onState({ phase: "authenticated", verification: null, error: null, sessions: [info], attached: info });
  }
  sendInput(data: string) { return mocks.sendInput(data); }
} }));
vi.mock("@xterm/xterm", () => ({ Terminal: class {
  input: ((data: string) => void) | null = null;
  constructor(public options: { disableStdin: boolean }) { mocks.terminals.push(this); }
  open() {}
  write() {}
  dispose() {}
  onData(callback: (data: string) => void) { this.input = callback; return { dispose() {} }; }
} }));

vi.mock("vue", async () => ({
  ...await vi.importActual<typeof import("vue")>("vue"),
  useSSRContext: () => ({ modules: new Set() }),
  onMounted: (callback: () => void) => { mocks.mounted = callback; },
  onBeforeUnmount: (callback: () => void) => { mocks.unmounted = callback; },
}));
let scope: EffectScope | undefined;
beforeEach(() => {
  mocks.terminals.length = 0; mocks.sendInput.mockReset().mockResolvedValue(undefined);
  vi.stubGlobal("window", { location: { hash: "", pathname: "/", search: "", protocol: "http:", host: "localhost" }, history: { replaceState() {} } });
  vi.stubGlobal("navigator", { platform: "test" });
  vi.stubGlobal("sessionStorage", { getItem: vi.fn(() => null), setItem: vi.fn(), removeItem: vi.fn() });
});
afterEach(() => { mocks.unmounted?.(); scope?.stop(); vi.unstubAllGlobals(); });
async function attach() {
  // Run the actual SFC setup with lifecycle hooks captured; Chromium covers DOM focus.
  scope = effectScope();
  const component = MobileApp as unknown as { setup: (props: object, context: { expose: () => void }) => {
    terminalHost: { value: unknown }; selectSession: (event: Event) => Promise<void>;
  } };
  const setup = scope.run(() => component.setup({}, { expose() {} }))!;
  setup.terminalHost.value = { style: {} };
  mocks.mounted!();
  await nextTick();
  await setup.selectSession({ target: { value: info.id } } as unknown as Event);
  await nextTick();
  return mocks.terminals[0];
}
describe("mobile terminal keyboard", () => {
  it("retains the native return marker while passing the invitation to the existing client", async () => {
    window.location.hash = "#invite=example-invitation&native=android";
    await attach();
    expect(sessionStorage.setItem).toHaveBeenCalledWith("winmux:native-remote:v1", "android");
    expect(mocks.options!.invite).toBe("example-invitation");
  });
  it("enables terminal typing and forwards consecutive keys before earlier acknowledgments", async () => {
    const terminal = await attach();
    expect(terminal.options.disableStdin).toBe(false);
    expect(terminal.input).toBeTypeOf("function");
    let acknowledge!: () => void;
    mocks.sendInput.mockImplementationOnce(() => new Promise<void>(resolve => { acknowledge = resolve; }));
    terminal.input!("a"); terminal.input!("b"); terminal.input!("\r");
    expect(mocks.sendInput.mock.calls).toEqual([["a"], ["b"], ["\r"]]);
    acknowledge();
  });
  it("blocks terminal input after disconnection", async () => {
    const terminal = await attach();
    expect(terminal.input).toBeTypeOf("function");
    mocks.options!.onState({ phase: "disconnected", verification: null, error: null, sessions: [info], attached: null });
    await nextTick();
    expect(terminal.options.disableStdin).toBe(true);
    terminal.input!("unexpected");
    expect(mocks.sendInput).not.toHaveBeenCalled();
  });
});
