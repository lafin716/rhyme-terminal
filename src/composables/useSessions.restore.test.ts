import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { nextTick } from "vue";

const bridge = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: bridge.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers();
  const values = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  });
  vi.stubGlobal("window", { setTimeout, clearTimeout });
  bridge.invoke.mockReset();
  bridge.invoke.mockImplementation(async (command, args) => {
    if (command === "list_sessions") return [];
    if (command === "create_session") return {
      id: "new-session", name: args.name, shell: args.shell,
      cwd: args.cwd, cols: 80, rows: 24, agent: "terminal",
    };
  });
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

type Agent = "claude" | "codex";
type TerminalPreset = "windows-powershell" | "zsh";
type RestoreFixture = {
  agent: Agent;
  savedPreset: TerminalPreset;
  hostPlatform: string;
  fallbackPreset: TerminalPreset;
};

function agentCommand(agent: Agent): string {
  return agent === "claude" ? "claude --permission-mode auto" : "codex --approve-for-me";
}

function expectLaunchArgs(shellArgs: string[], agent: Agent, preset: TerminalPreset) {
  const command = agentCommand(agent);
  if (preset === "windows-powershell") {
    const lastArg = shellArgs.slice(-1)[0];
    expect(shellArgs.slice(0, 2)).toEqual(["-NoExit", "-Command"]);
    expect(lastArg).toContain("function global:prompt");
    expect(lastArg).toMatch(new RegExp(`; ${command.replace(/ /g, "\\s")}$`));
    return;
  }
  expect(shellArgs).toEqual(["-l", "-i", "-c", `${command}; exec /bin/zsh -l -i`]);
}

it.each([
  { agent: "claude", savedPreset: "windows-powershell", hostPlatform: "Win32", fallbackPreset: "windows-powershell" },
  { agent: "codex", savedPreset: "windows-powershell", hostPlatform: "Win32", fallbackPreset: "windows-powershell" },
  { agent: "claude", savedPreset: "windows-powershell", hostPlatform: "MacIntel", fallbackPreset: "zsh" },
  { agent: "codex", savedPreset: "windows-powershell", hostPlatform: "MacIntel", fallbackPreset: "zsh" },
  { agent: "claude", savedPreset: "zsh", hostPlatform: "MacIntel", fallbackPreset: "zsh" },
  { agent: "codex", savedPreset: "zsh", hostPlatform: "MacIntel", fallbackPreset: "zsh" },
] satisfies RestoreFixture[])(
  "restores $agent with $savedPreset and $hostPlatform fallback through shell fallback",
  async ({ agent, savedPreset, hostPlatform, fallbackPreset }) => {
  vi.stubGlobal("navigator", { platform: hostPlatform });
  const { useWorkspaces } = await import("./useWorkspaces");
  const { useSessions } = await import("./useSessions");
  const { useAccountProfiles } = await import("./useAccountProfiles");
  const { configForPreset } = await import("../lib/terminal-config");
  useAccountProfiles().profiles.push({
    id: "work", agent, label: "Work", configDir: "C:\\accounts\\work", createdAt: 1,
    authMethod: "setup-token", env: { HTTP_PROXY: "http://localhost:8080" },
  });
  const createRequests: Record<string, unknown>[] = [];
  bridge.invoke.mockImplementation(async (command, args) => {
    if (command === "get_account_token") return "test-token";
    if (command === "create_session") {
      createRequests.push(args);
      if (createRequests.length === 1) throw new Error("cwd unavailable");
      return { id: "restored", name: args.name, shell: args.shell, cols: 80, rows: 24, agent: "terminal" };
    }
  });
  const sessions = useSessions();
  const ws = useWorkspaces().activeWorkspace.value;
  const restored = await sessions.restoreForWorkspace(ws, {
    name: "project", agent, cwd: "C:\\project", terminal: configForPreset(savedPreset),
    accountProfile: { agent, id: "work", label: "Work" },
  });
  expect(restored?.id).toBe("restored");
  expect(createRequests).toHaveLength(2);
  expect(createRequests[0].cwd).toBe("C:\\project");
  expect(createRequests[1].cwd).toBeUndefined();
  expectLaunchArgs(createRequests[0].shellArgs as string[], agent, savedPreset);
  expectLaunchArgs(createRequests[1].shellArgs as string[], agent, fallbackPreset);
  for (const request of createRequests) {
    expect(request.env).toMatchObject({
      [agent === "claude" ? "CLAUDE_CONFIG_DIR" : "CODEX_HOME"]: "C:\\accounts\\work",
      HTTP_PROXY: "http://localhost:8080",
      ...(agent === "claude" ? { CLAUDE_CODE_OAUTH_TOKEN: "test-token" } : {}),
    });
  }
  expect(ws.terminalSnapshots.restored).toMatchObject({ agent, accountProfile: { id: "work" } });
  expect(JSON.stringify(ws.terminalSnapshots)).not.toContain("test-token");
});

it("restores legacy profile snapshots but respects an explicit terminal state", async () => {
  const { useWorkspaces } = await import("./useWorkspaces");
  const { useSessions } = await import("./useSessions");
  const { configForPreset } = await import("../lib/terminal-config");
  const sessions = useSessions();
  const ws = useWorkspaces().activeWorkspace.value;
  const snapshot = {
    name: "old", terminal: configForPreset("windows-powershell"),
    accountProfile: { agent: "codex" as const, id: null, label: "" },
  };
  await sessions.restoreForWorkspace(ws, snapshot);
  expectLaunchArgs(bridge.invoke.mock.calls.slice(-1)[0]?.[1].shellArgs, "codex", "windows-powershell");
  await sessions.restoreForWorkspace(ws, { ...snapshot, agent: "terminal" });
  expect(bridge.invoke.mock.calls.slice(-1)[0]?.[1].shellArgs.slice(-1)[0]).not.toContain("codex");
});

it("keeps a missing saved account from being replaced by the system account", async () => {
  const { useWorkspaces } = await import("./useWorkspaces");
  const { useSessions } = await import("./useSessions");
  const { defaultTerminalConfig } = await import("../lib/terminal-config");
  const restored = await useSessions().restoreForWorkspace(useWorkspaces().activeWorkspace.value, {
    name: "old", agent: "claude", terminal: defaultTerminalConfig(),
    accountProfile: { agent: "claude", id: "missing", label: "Deleted" },
  });
  expect(restored).toBeNull();
  expect(bridge.invoke).not.toHaveBeenCalled();
});

it("backfills live daemon agent state and preserves launch intent during startup polling", async () => {
  const { useWorkspaces } = await import("./useWorkspaces");
  const { useSessions } = await import("./useSessions");
  const sessions = useSessions();
  await sessions.create({ launchCommand: "claude" });
  const ws = useWorkspaces().activeWorkspace.value;
  bridge.invoke.mockResolvedValue([{ ...sessions.state.sessions[0], agent: "terminal" }]);
  await sessions.refresh();
  expect(ws.terminalSnapshots["new-session"].agent).toBe("claude");
  bridge.invoke.mockResolvedValue([{ ...sessions.state.sessions[0], agent: "codex" }]);
  await sessions.refresh();
  expect(ws.terminalSnapshots["new-session"].agent).toBe("codex");
  bridge.invoke.mockResolvedValue([{ ...sessions.state.sessions[0], agent: "terminal" }]);
  await sessions.refresh();
  expect(ws.terminalSnapshots["new-session"].agent).toBe("terminal");
});

it.each(["claude", "codex"] as const)("persists detected %s and clears it on return to the shell", async (agent) => {
  const { useWorkspaces, loadFromStorage } = await import("./useWorkspaces");
  const { useSessions } = await import("./useSessions");
  const { loadWorkspaces } = await import("../lib/persistence");
  const workspaces = useWorkspaces();
  const ws = workspaces.activeWorkspace.value;
  const sessions = useSessions();
  await sessions.create();
  sessions.applyAgentUpdate({ id: "new-session", agent });
  await nextTick();
  vi.advanceTimersByTime(250);
  expect(loadWorkspaces()?.workspaces[0].terminalSnapshots["new-session"]).toMatchObject({ agent });
  loadFromStorage();
  expect(workspaces.activeWorkspace.value.terminalSnapshots["new-session"]).toMatchObject({ agent });
  sessions.applyAgentUpdate({ id: "new-session", agent: "terminal" });
  await nextTick();
  vi.advanceTimersByTime(250);
  expect(loadWorkspaces()?.workspaces[0].terminalSnapshots["new-session"]).toMatchObject({ agent: "terminal" });
  expect(ws.layout.kind).toBe("leaf");
});

it.each(["claude", "codex"] as const)("saves %s launch intent before process detection", async (agent) => {
  const { useWorkspaces } = await import("./useWorkspaces");
  const { useSessions } = await import("./useSessions");
  const sessions = useSessions();
  await sessions.create({ launchCommand: agent });
  expect(useWorkspaces().activeWorkspace.value.terminalSnapshots["new-session"]).toMatchObject({ agent });
});

it("does not resurrect an agent that already exited before create returned", async () => {
  const { useWorkspaces } = await import("./useWorkspaces");
  const { useSessions } = await import("./useSessions");
  const sessions = useSessions();
  bridge.invoke.mockImplementation(async (_command, args) => {
    sessions.applyAgentUpdate({ id: "fast-exit", agent: "claude" });
    sessions.applyAgentUpdate({ id: "fast-exit", agent: "terminal" });
    return { id: "fast-exit", name: args.name, shell: args.shell, cols: 80, rows: 24, agent: "terminal" };
  });
  await sessions.create({ launchCommand: "claude" });
  expect(useWorkspaces().activeWorkspace.value.terminalSnapshots["fast-exit"].agent).toBe("terminal");
});
