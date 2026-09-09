import { t } from "./useI18n";
import { resolveProfileEnv, useAccountProfiles } from "./useAccountProfiles";
import { profileForLaunch } from "../lib/session-profile";
import { sessionAccountEnv } from "../lib/account-env";
import { reactive, computed, watch } from "vue";
import {
  api,
  onSessionActivity,
  onSessionAgentStatusChanged,
  type SessionAgentPayload,
  type AgentTaskStatus,
  type SessionInfo,
} from "../lib/tauri";
import {
  applyActivity,
  clearActivity,
  type ActivityState,
  type SessionActivityFlags,
} from "../lib/session-activity";
import {
  applySessionAgent,
  applySessionAgentUpdates,
  type VersionedSessionAgentPayload,
} from "../lib/session-agent";
import type { TerminalTabSnapshot, Workspace } from "../lib/layout-types";
import { useWorkspaces, workspaceDefaultCwd } from "./useWorkspaces";
import { useFocus } from "./useFocus";
import { usePrefs } from "./usePrefs";
import { cloneTerminalConfig } from "../lib/terminal-config";
import type { TerminalConfig } from "../lib/terminal-config";
import { withAgentLaunch, withCwdIntegration } from "../lib/terminal-launch";
import { displayName, withWorkspacePrefix, workspaceIndexOf } from "../lib/session-names";
import {
  addTabToLeaf,
  collectAllSessionIds,
  findFirstLeaf,
  findLeafById,
  findLeafBySession,
  removeTab,
} from "./useLayout";

// Re-exported so existing importers keep resolving the name helpers from here;
// the scheme itself lives in the pure `../lib/session-names` module.
export { displayName, withWorkspacePrefix };

interface Store {
  sessions: SessionInfo[];
}

const state = reactive<Store>({
  sessions: [],
});
const currentCwds = reactive<Record<string, string>>({});

// Transient, in-memory per-Session activity flags (mirrors the `currentCwds`
// pattern). Fed by the single app-wide `session-activity` listener below and
// cleared when a Session becomes focused; never persisted. The pure reducer in
// `session-activity.ts` returns new objects, so `commitActivity` reconciles the
// reactive map in place to keep readers (Navigator badges, Ticket 2) reactive.
const activity = reactive<ActivityState>({});
const agentTaskStatus = reactive<Record<string, AgentTaskStatus>>({});
let activityListenerStarted = false;
let agentTaskStatusListenerStarted = false;
let sessionAgentRevision = 0;
const latestSessionAgentUpdates = new Map<string, VersionedSessionAgentPayload>();
const pendingAgentLaunches = new Set<string>();

function saveAgentSnapshot(payload: SessionAgentPayload): void {
  const { state: workspaces, updateTerminalSnapshot } = useWorkspaces();
  for (const ws of workspaces.workspaces) {
    const snapshot = ws.terminalSnapshots[payload.id];
    if (!snapshot || snapshot.agent === payload.agent) continue;
    updateTerminalSnapshot(ws.id, payload.id, { agent: payload.agent });
  }
}

function applyAgentUpdate(payload: SessionAgentPayload): void {
  const update = { revision: ++sessionAgentRevision, payload };
  latestSessionAgentUpdates.set(payload.id, update);
  state.sessions = applySessionAgent(state.sessions, payload);
  pendingAgentLaunches.delete(payload.id);
  saveAgentSnapshot(payload);
}

function reconcileAgentUpdates(
  sessions: SessionInfo[],
  snapshotRevision: number,
): SessionInfo[] {
  return applySessionAgentUpdates(
    sessions,
    latestSessionAgentUpdates.values(),
    snapshotRevision,
  );
}

function commitActivity(next: ActivityState) {
  for (const id of Object.keys(activity)) {
    if (!(id in next)) delete activity[id];
  }
  for (const id of Object.keys(next)) {
    const n = next[id];
    const cur = activity[id];
    if (!cur || cur.output !== n.output || cur.bell !== n.bell) {
      activity[id] = { output: n.output, bell: n.bell };
    }
  }
}

const SESSION_NUM_RE = /^session-(\d+)$/;

export function nextDaemonName(ws: Workspace, sessions: SessionInfo[]): string {
  const usedIds = new Set(collectAllSessionIds(ws.layout));
  const usedNums = new Set<number>();
  for (const s of sessions) {
    if (!usedIds.has(s.id)) continue;
    const m = displayName(s.name).match(SESSION_NUM_RE);
    if (m) usedNums.add(parseInt(m[1], 10));
  }
  let n = 1;
  while (usedNums.has(n)) n++;
  return `w${ws.index}.session-${n}`;
}

export function useSessions() {
  const {
    activeWorkspace,
    replaceLayout,
    removeTerminalSnapshot,
    setTerminalSnapshot,
    state: wsState,
    updateTerminalSnapshot,
  } = useWorkspaces();
  const { focusedLeafId, setFocusedLeaf } = useFocus();
  const { prefs } = usePrefs();

  function getById(id: string): SessionInfo | undefined {
    return state.sessions.find((s) => s.id === id);
  }

  const focusedSession = computed<SessionInfo | null>(() => {
    const ws = activeWorkspace.value;
    if (!ws || !focusedLeafId.value) return null;
    const leaf = findLeafById(ws.layout, focusedLeafId.value);
    if (!leaf || !leaf.activeTabId) return null;
    return getById(leaf.activeTabId) ?? null;
  });

  // Register the single, app-wide activity pipeline once (not per component):
  // fold each `session-activity` event with the focused Session treated as
  // already-seen, and clear a Session's flags when it gains focus.
  if (!activityListenerStarted) {
    activityListenerStarted = true;
    void onSessionActivity((payload) => {
      const focusedId = focusedSession.value?.id ?? null;
      commitActivity(applyActivity(activity, payload, focusedId));
    });
    watch(focusedSession, (session) => {
      if (session) commitActivity(clearActivity(activity, session.id));
    });
  }

  if (!agentTaskStatusListenerStarted) {
    agentTaskStatusListenerStarted = true;
    void onSessionAgentStatusChanged((payload) => {
      agentTaskStatus[payload.id] = payload.status;
    });
  }

  const workspaceSessions = computed<SessionInfo[]>(() => {
    const ws = activeWorkspace.value;
    if (!ws) return [];
    const ids = collectAllSessionIds(ws.layout);
    const out: SessionInfo[] = [];
    for (const id of ids) {
      const s = state.sessions.find((x) => x.id === id);
      if (s) out.push(s);
    }
    return out;
  });

  async function refresh() {
    const snapshotRevision = sessionAgentRevision;
    state.sessions = reconcileAgentUpdates(await api.listSessions(), snapshotRevision);
    for (const session of state.sessions) {
      if (session.cwd && !currentCwds[session.id]) currentCwds[session.id] = session.cwd;
      if (session.agent !== "terminal" || !pendingAgentLaunches.has(session.id)) {
        pendingAgentLaunches.delete(session.id);
        saveAgentSnapshot(session);
      }
    }
  }

  async function createForWorkspace(
    ws: Workspace | null | undefined,
    opts: {
      name?: string;
      shell?: string;
      shellArgs?: string[];
      cwd?: string;
      terminal?: TerminalConfig;
      showError?: boolean;
      env?: Record<string, string>;
      launchCommand?: string;
    } = {},
  ): Promise<SessionInfo | null> {
    const requestedName = opts.name ?? (ws ? nextDaemonName(ws, state.sessions) : undefined);
    const name = ws && requestedName
      ? withWorkspacePrefix(requestedName, ws.index)
      : requestedName;
    const cwd = opts.cwd ?? workspaceDefaultCwd(ws);
    const terminal = cloneTerminalConfig(
      opts.terminal ?? ws?.settings?.terminal ?? prefs.defaultTerminal,
    );
    const shell = opts.shell ?? terminal.program.trim();
    const requestedArgs = opts.shell
      ? (opts.shellArgs ?? [])
      : (opts.shellArgs ?? terminal.args);
    const argsForHook = [...requestedArgs];
    const hookedArgs = opts.shell ? argsForHook : withCwdIntegration(terminal, argsForHook);
    const shellArgs = !opts.shell && opts.launchCommand
      ? withAgentLaunch(terminal, hookedArgs, argsForHook, opts.launchCommand)
      : hookedArgs;
    if (!shell) {
      alert(t("Select a terminal program in Settings before creating a session."));
      return null;
    }
    try {
      const snapshotRevision = sessionAgentRevision;
      const [info] = reconcileAgentUpdates([await api.createSession({
        name,
        shell,
        shellArgs: [...shellArgs],
        cwd,
        env: sessionAccountEnv(opts.launchCommand, opts.env, prefs.systemAccountEnv),
      })], snapshotRevision);
      // A detected transition (including an early exit) wins over launch intent.
      const awaitingAgent = info.agent === "terminal"
        && (opts.launchCommand === "claude" || opts.launchCommand === "codex")
        && (latestSessionAgentUpdates.get(info.id)?.revision ?? 0) <= snapshotRevision;
      state.sessions.push(info);
      if (info.cwd) currentCwds[info.id] = info.cwd;
      if (ws) {
        setTerminalSnapshot(ws.id, info.id, {
          agent: awaitingAgent ? opts.launchCommand as "claude" | "codex" : info.agent,
          accountProfile: profileForLaunch(useAccountProfiles().profiles, opts.env, opts.launchCommand),
          name: displayName(info.name),
          terminal,
          cwd: info.cwd ?? cwd ?? null,
        });
      }
      if (awaitingAgent) {
        pendingAgentLaunches.add(info.id);
      }
      return info;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (opts.showError === false) {
        console.warn("Failed to create terminal", message);
      } else {
        alert(t("Failed to create terminal: {message}", { message }));
      }
      return null;
    }
  }

  async function restoreForWorkspace(ws: Workspace, snapshot: TerminalTabSnapshot): Promise<SessionInfo | null> {
    const agent = snapshot.agent ?? snapshot.accountProfile?.agent ?? "terminal";
    const launchCommand = agent === "claude" || agent === "codex" ? agent : undefined;
    let env: Record<string, string> | undefined;
    const account = snapshot.accountProfile;
    if (launchCommand && account?.agent === agent && account.id) {
      const profile = useAccountProfiles().profiles.find(p => p.id === account.id && p.agent === agent);
      // Do not silently switch an unavailable saved account to the system account.
      if (!profile) {
        console.warn(`Cannot restore terminal: saved account profile ${account.id} is unavailable`);
        return null;
      }
      try {
        env = await resolveProfileEnv(profile);
      } catch {
        console.warn("Cannot restore terminal: failed to resolve saved account profile");
        return null;
      }
    }
    const opts = { name: snapshot.name, launchCommand, env, showError: false };
    return await createForWorkspace(ws, {
      ...opts, terminal: snapshot.terminal, cwd: snapshot.cwd ?? undefined,
    }) ?? await createForWorkspace(ws, opts);
  }

  async function create(opts: {
    name?: string;
    shell?: string;
    shellArgs?: string[];
    cwd?: string;
    terminal?: TerminalConfig;
    showError?: boolean;
    env?: Record<string, string>;
    launchCommand?: string;
  } = {}) {
    const ws = activeWorkspace.value;
    const info = await createForWorkspace(ws, opts);
    if (!info) return null;
    if (ws) {
      const targetLeafId =
        focusedLeafId.value && findLeafById(ws.layout, focusedLeafId.value)
          ? focusedLeafId.value
          : findFirstLeaf(ws.layout).id;
      addTabToLeaf(ws.layout, targetLeafId, info.id);
      setFocusedLeaf(targetLeafId);
    }
    return info;
  }

  async function kill(id: string) {
    await api.killSession(id);
    const idx = state.sessions.findIndex((s) => s.id === id);
    if (idx >= 0) state.sessions.splice(idx, 1);
    latestSessionAgentUpdates.delete(id);
    pendingAgentLaunches.delete(id);
    delete currentCwds[id];
    delete activity[id];
    delete agentTaskStatus[id];
    for (const ws of wsState.workspaces) {
      if (findLeafBySession(ws.layout, id)) {
        const { root } = removeTab(ws.layout, id);
        if (root !== ws.layout) replaceLayout(ws.id, root);
      }
      removeTerminalSnapshot(ws.id, id);
    }
  }

  async function rename(id: string, name: string) {
    const s = state.sessions.find((x) => x.id === id);
    if (!s) return;
    const index = workspaceIndexOf(s.name);
    const ws = wsState.workspaces.find(w => w.index === index);
    const prefix = index !== null ? `w${index}.` : "";
    const fullName = `${prefix}${displayName(name)}`;
    await api.renameSession(id, fullName);
    if (s) s.name = fullName;
    if (ws) updateTerminalSnapshot(ws.id, id, { name: displayName(fullName) });
  }

  return {
    state,
    focusedSession,
    workspaceSessions,
    refresh,
    create,
    createForWorkspace,
    restoreForWorkspace,
    kill,
    rename,
    getById,
    applyAgentUpdate,
    activity,
    agentTaskStatus,
    sessionAgentStatus(id: string): AgentTaskStatus | undefined {
      return agentTaskStatus[id];
    },
    sessionActivity(id: string): SessionActivityFlags | undefined {
      return activity[id];
    },
    currentCwd(id: string): string | undefined {
      return currentCwds[id] ?? getById(id)?.cwd ?? undefined;
    },
    setCurrentCwd(id: string, cwd: string) {
      const clean = cwd.trim();
      if (!clean) return;
      currentCwds[id] = clean;
      for (const ws of wsState.workspaces) {
        if (findLeafBySession(ws.layout, id)) {
          updateTerminalSnapshot(ws.id, id, { cwd: clean });
          break;
        }
      }
    },
  };
}
