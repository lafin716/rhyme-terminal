import { reactive, computed, watch } from "vue";
import type {
  LayoutNode,
  TerminalTabSnapshot,
  Workspace,
  WorkspaceSettings,
  WorkspaceStore,
} from "../lib/layout-types";
import { makeLeaf, nodeId } from "../lib/layout-types";
import { loadWorkspaces, saveWorkspaces } from "../lib/persistence";
import { cloneTerminalConfig, normalizeTerminalConfig } from "../lib/terminal-config";

function defaultWorkspaceSettings(): WorkspaceSettings {
  return {
    defaultCwd: "",
    terminal: null,
  };
}

function makeDefaultWorkspace(name = "Default", icon = "W", index = 1): Workspace {
  return {
    id: nodeId("ws"),
    name,
    icon,
    index,
    nextSessionSeq: 1,
    settings: defaultWorkspaceSettings(),
    layout: makeLeaf(nodeId("leaf")),
    terminalSnapshots: {},
  };
}

function backfillWorkspace(ws: Workspace) {
  if (typeof ws.index !== "number") return;
  if (typeof ws.nextSessionSeq !== "number") ws.nextSessionSeq = 1;
  ws.settings = {
    ...defaultWorkspaceSettings(),
    ...(ws.settings ?? {}),
  };
  if (ws.settings.terminal) {
    ws.settings.terminal = normalizeTerminalConfig(ws.settings.terminal);
  }
  const snapshots: Record<string, TerminalTabSnapshot> = {};
  for (const [id, snapshot] of Object.entries(ws.terminalSnapshots ?? {})) {
    if (!snapshot?.terminal) continue;
    snapshots[id] = {
      agent: snapshot.agent === "claude" || snapshot.agent === "codex" || snapshot.agent === "terminal"
        ? snapshot.agent : snapshot.accountProfile?.agent ?? "terminal",
      name: typeof snapshot.name === "string" && snapshot.name.trim()
        ? snapshot.name.trim()
        : "session",
      cwd: typeof snapshot.cwd === "string" && snapshot.cwd.trim()
        ? snapshot.cwd.trim()
        : null,
      accountProfile: snapshot.accountProfile,
      terminal: normalizeTerminalConfig(snapshot.terminal),
    };
  }
  ws.terminalSnapshots = snapshots;
}

function defaultStore(): WorkspaceStore {
  const ws = makeDefaultWorkspace();
  return { workspaces: [ws], activeWorkspaceId: ws.id };
}

function nextWorkspaceIndex(): number {
  let max = 0;
  for (const w of state.workspaces) {
    if (typeof w.index === "number" && w.index > max) max = w.index;
  }
  return max + 1;
}

const state = reactive<WorkspaceStore>(defaultStore());

let loaded = false;
let saveTimer: number | null = null;

function scheduleSave() {
  if (saveTimer !== null) window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    saveWorkspaces({
      workspaces: state.workspaces,
      activeWorkspaceId: state.activeWorkspaceId,
    });
    saveTimer = null;
  }, 200);
}

watch(state, scheduleSave, { deep: true });

export function loadFromStorage() {
  if (loaded) return;
  loaded = true;
  const stored = loadWorkspaces();
  if (stored && stored.workspaces.length > 0) {
    // Backfill missing index/nextSessionSeq from older persisted stores.
    let usedMax = 0;
    for (const w of stored.workspaces) {
      if (typeof w.index === "number" && w.index > usedMax) usedMax = w.index;
    }
    for (const w of stored.workspaces) {
      if (typeof w.index !== "number") w.index = ++usedMax;
      backfillWorkspace(w);
    }
    state.workspaces = stored.workspaces;
    state.activeWorkspaceId =
      stored.workspaces.find((w) => w.id === stored.activeWorkspaceId)?.id ??
      stored.workspaces[0].id;
  }
}

export function useWorkspaces() {
  const activeWorkspace = computed(
    () =>
      state.workspaces.find((w) => w.id === state.activeWorkspaceId) ??
      state.workspaces[0],
  );

  function createWorkspace(name = "Workspace", icon?: string): Workspace {
    const ws = makeDefaultWorkspace(name, icon ?? name.slice(0, 1).toUpperCase(), nextWorkspaceIndex());
    state.workspaces.push(ws);
    state.activeWorkspaceId = ws.id;
    return ws;
  }

  function deleteWorkspace(id: string) {
    if (state.workspaces.length <= 1) return;
    const idx = state.workspaces.findIndex((w) => w.id === id);
    if (idx < 0) return;
    state.workspaces.splice(idx, 1);
    if (state.activeWorkspaceId === id) {
      state.activeWorkspaceId = state.workspaces[Math.max(0, idx - 1)].id;
    }
  }

  function renameWorkspace(id: string, name: string) {
    const ws = state.workspaces.find((w) => w.id === id);
    if (ws && name.trim()) {
      ws.name = name.trim();
      if (!ws.icon || ws.icon.length <= 1) {
        ws.icon = name.trim().slice(0, 1).toUpperCase();
      }
    }
  }

  function setActiveWorkspace(id: string) {
    if (state.workspaces.find((w) => w.id === id)) {
      state.activeWorkspaceId = id;
    }
  }

  function replaceLayout(workspaceId: string, layout: LayoutNode) {
    const ws = state.workspaces.find((w) => w.id === workspaceId);
    if (ws) ws.layout = layout;
  }

  function updateWorkspaceSettings(id: string, patch: Partial<WorkspaceSettings>) {
    const ws = state.workspaces.find((w) => w.id === id);
    if (!ws) return;
    ws.settings = {
      ...defaultWorkspaceSettings(),
      ...(ws.settings ?? {}),
      ...patch,
    };
  }

  function setTerminalSnapshot(
    workspaceId: string,
    sessionId: string,
    snapshot: TerminalTabSnapshot,
  ) {
    const ws = state.workspaces.find((w) => w.id === workspaceId);
    if (!ws) return;
    ws.terminalSnapshots[sessionId] = {
      agent: snapshot.agent,
      name: snapshot.name.trim() || "session",
      cwd: snapshot.cwd?.trim() || null,
      accountProfile: snapshot.accountProfile,
      terminal: cloneTerminalConfig(snapshot.terminal),
    };
  }

  function updateTerminalSnapshot(
    workspaceId: string,
    sessionId: string,
    patch: Partial<TerminalTabSnapshot>,
  ) {
    const ws = state.workspaces.find((w) => w.id === workspaceId);
    const current = ws?.terminalSnapshots[sessionId];
    if (!ws || !current) return;
    ws.terminalSnapshots[sessionId] = {
      ...current,
      ...patch,
      terminal: patch.terminal ? cloneTerminalConfig(patch.terminal) : current.terminal,
      name: (patch.name ?? current.name).trim() || "session",
      cwd: patch.cwd === undefined ? current.cwd : (patch.cwd?.trim() || null),
    };
  }

  function removeTerminalSnapshot(workspaceId: string, sessionId: string) {
    const ws = state.workspaces.find((w) => w.id === workspaceId);
    if (!ws) return;
    delete ws.terminalSnapshots[sessionId];
  }

  return {
    state,
    activeWorkspace,
    createWorkspace,
    deleteWorkspace,
    renameWorkspace,
    setActiveWorkspace,
    replaceLayout,
    updateWorkspaceSettings,
    setTerminalSnapshot,
    updateTerminalSnapshot,
    removeTerminalSnapshot,
  };
}

export function workspaceDefaultCwd(ws: Workspace | null | undefined): string | undefined {
  const cwd = ws?.settings?.defaultCwd?.trim();
  return cwd ? cwd : undefined;
}
