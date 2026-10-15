import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), setPref: vi.fn(), profiles: [
  { id: "work", agent: "claude", label: "Work", authMethod: "oauth" },
  { id: "token", agent: "claude", label: "Token", authMethod: "setup-token" },
], prefs: { toolbarProfileId: {} as Partial<Record<"claude" | "codex", string | null>>, defaultProfileId: { claude: "work", codex: null } } }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("./useAccountProfiles", () => ({
  CLI_AGENTS: [{ id: "claude" }, { id: "codex" }],
  useAccountProfiles: () => ({ profiles: mocks.profiles }),
}));
vi.mock("./usePrefs", async () => {
  const { reactive } = await import("vue");
  const prefs = reactive(mocks.prefs);
  mocks.setPref.mockImplementation((key: "toolbarProfileId", value) => { prefs[key] = value; });
  return { usePrefs: () => ({ prefs, setPref: mocks.setPref }) };
});

beforeEach(() => {
  vi.resetModules();
  mocks.invoke.mockReset();
  mocks.setPref.mockClear();
  mocks.prefs.toolbarProfileId = {};
  vi.spyOn(Date, "now").mockReturnValue(1_000_000);
});

afterEach(() => vi.restoreAllMocks());

describe("profile usage", () => {
  it("polls setup-token samples locally and retains their actual receipt time", async () => {
    mocks.invoke.mockResolvedValue([{ label: "5h", percentUsed: 12, resetsAt: 1_020_000, receivedAt: 990_000 }]);
    const { useUsage } = await import("./useUsage");
    const state = useUsage();
    await state.refresh(true);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(mocks.invoke).toHaveBeenCalledWith("get_account_usage", { agent: "claude", profileId: "token", sessionUsage: true });
    expect(state.records["claude:token"].updatedAt).toBe(990_000);
    vi.spyOn(Date, "now").mockReturnValue(1_010_000);
    mocks.invoke.mockRejectedValue("Waiting for session usage. Start this profile and send a message");
    await state.refresh(true);
    expect(state.records["claude:token"].windows).toEqual([]);
    expect(state.records["claude:token"].error).toContain("Waiting");
  });
  it("switches toolbar profiles independently and persists selection without changing launch defaults", async () => {
    mocks.invoke.mockImplementation((_command, args) => Promise.resolve([{ label: "5h", percentUsed: args.profileId === "work" ? 42 : 10, resetsAt: null }]));
    const { useUsage } = await import("./useUsage");
    const state = useUsage();
    await state.refresh();
    expect(state.usage.value.claude.percentUsed).toBe(42);
    state.selectToolbarProfile("claude", null);
    expect(state.usage.value.claude.percentUsed).toBe(10);
    expect(mocks.setPref).toHaveBeenLastCalledWith("toolbarProfileId", { claude: null });
    expect(mocks.prefs.defaultProfileId.claude).toBe("work");
    state.selectToolbarProfile("codex", null);
    state.selectToolbarProfile("claude", "work");
    expect(state.usage.value.claude.percentUsed).toBe(42);
    expect(mocks.setPref).toHaveBeenLastCalledWith("toolbarProfileId", { claude: "work", codex: null });
    state.selectToolbarProfile("codex", "work");
    expect(state.selected("codex").id).toBeNull();
    expect(mocks.setPref).toHaveBeenCalledTimes(3);
  });
  it("updates already-rendered computed values when the first request completes", async () => {
    let release!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    mocks.invoke.mockImplementation(async () => {
      await gate;
      return [{ label: "5h", percentUsed: 37, resetsAt: null }];
    });
    const { useUsage } = await import("./useUsage");
    const state = useUsage();
    const pending = state.refresh();
    expect(state.usage.value.claude.percentUsed).toBeNull();
    expect(state.loading.value).toBe(true);
    release();
    await pending;
    expect(state.usage.value.claude.percentUsed).toBe(37);
    expect(state.loading.value).toBe(false);
  });
  it("loads system and OAuth profiles independently and summarizes the selected profile", async () => {
    mocks.invoke.mockImplementation((_command, args) => Promise.resolve([{ label: "5h", percentUsed: args.profileId === "work" ? 42 : 10, resetsAt: "2026-09-08T00:00:00Z" }]));
    const { useUsage } = await import("./useUsage");
    const state = useUsage();
    await state.refresh();
    expect(mocks.invoke).toHaveBeenCalledTimes(4);
    expect(state.usage.value.claude.percentUsed).toBe(42);
    expect(state.usage.value.codex.percentUsed).toBe(10);
    expect(state.usage.value.claude.resetsAt).toBe(Date.parse("2026-09-08T00:00:00Z"));
    expect(mocks.invoke).toHaveBeenCalledWith("get_account_usage", { agent: "claude", profileId: "token", sessionUsage: true });
    await state.refresh();
    expect(mocks.invoke).toHaveBeenCalledTimes(4);
  });

  it("retains successful data on refresh failure and isolates profile failures", async () => {
    mocks.invoke.mockResolvedValue([{ label: "5h", percentUsed: 0, resetsAt: null }]);
    const { useUsage } = await import("./useUsage");
    const state = useUsage();
    await state.refresh();
    vi.spyOn(Date, "now").mockReturnValue(1_061_000);
    mocks.invoke.mockImplementation((_command, args) => args.agent === "claude"
      ? Promise.reject("Too many requests. Try again later")
      : Promise.resolve([{ label: "5h", percentUsed: 20, resetsAt: null }]));
    await state.refresh();
    expect(state.usage.value.claude.percentUsed).toBe(0);
    expect(state.records["claude:work"].updatedAt).toBe(1_000_000);
    expect(state.records["claude:work"].error).toContain("Too many");
    expect(state.usage.value.codex.percentUsed).toBe(20);
    expect(state.loading.value).toBe(false);
  });
});
