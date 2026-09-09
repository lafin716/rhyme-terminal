import { reactive, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { usePrefs } from './usePrefs';
import { useAccountProfiles } from './useAccountProfiles';
import { useWorkspaces } from './useWorkspaces';
import { addTabToLeaf, findFirstLeaf, findLeafBySession, removeTab } from './useLayout';
import { loopTabId, mergeLoopProfiles, type LoopGroup, type LoopSettings, type LoopStart, type LoopConversation } from '../lib/loop-routing';
const KEY = 'winmux:loop-routing:v1';
const state = reactive({ settings: { shortThreshold: 90, weeklyThreshold: 90, agentOrder: ['claude', 'codex'], candidates: [] } as LoopSettings, groups: [] as LoopGroup[], error: '', ready: false });
let timer: ReturnType<typeof setTimeout> | undefined;
let stopProfiles: (() => void) | undefined;
let closed = false;
let knownActiveSessions = new Set<string>();
let refreshGeneration = 0;
const request = <T>(request: object) => invoke<T>('loop_request', { request });
async function requireExplicitStart() {
  const capabilities = await request<{ explicitStart?: boolean }>({ op: 'capabilities' });
  if (!capabilities.explicitStart) throw new Error('실행 중인 데몬은 새 루프 생성 방식을 지원하지 않습니다. 작업을 마친 뒤 앱과 데몬을 재시작하세요.');
}
export async function saveLoopSettings(settings = state.settings) {
  const merged = mergeLoopProfiles(settings, useAccountProfiles().profiles);
  for (const candidate of merged.candidates) {
    if (candidate.profileId === null) candidate.env = { ...usePrefs().prefs.systemAccountEnv?.[candidate.agent] };
  }
  await request<LoopSettings>({ op: 'configure', settings: merged });
  state.settings = merged;
  localStorage.setItem(KEY, JSON.stringify({ version: 1, ...state.settings, candidates: state.settings.candidates.map(({ env: _env, ...candidate }) => candidate) }));
  state.error = '';
}
export async function refreshLoopGroups(skipId?: string) {
  const generation = ++refreshGeneration;
  const groups = await request<LoopGroup[]>({ op: 'list' });
  if (generation !== refreshGeneration) return;
  const activeSessions = groups.flatMap(group => group.activeSessionId ? [group.activeSessionId] : []);
  if (activeSessions.length !== knownActiveSessions.size || activeSessions.some(id => !knownActiveSessions.has(id))) {
    // The daemon changes managed PTYs without add/remove events. Refresh
    // their metadata for terminal cwd, hyperlinks, and file-drop resolution.
    const { useSessions } = await import('./useSessions');
    await useSessions().refresh();
    if (generation !== refreshGeneration) return;
  }
  knownActiveSessions = new Set(activeSessions);
  state.groups = groups;
  const { state: workspaces } = useWorkspaces();
  for (const group of state.groups) {
    const ws = workspaces.workspaces.find(w => w.id === group.workspaceId);
    if (!ws || group.status === 'stopped' || group.id === skipId) continue;
    const id = loopTabId(group.id);
    if (!findLeafBySession(ws.layout, id)) {
      const leaf = findFirstLeaf(ws.layout);
      const active = leaf.activeTabId;
      addTabToLeaf(ws.layout, leaf.id, id);
      if (active) leaf.activeTabId = active;
    }
  }
}
export async function initializeLoopRouting() {
  closed = false;
  try {
    const stored = JSON.parse(localStorage.getItem(KEY) ?? 'null');
    if (stored?.version === 1 && Array.isArray(stored.candidates) && Array.isArray(stored.agentOrder)) state.settings = stored;
    await saveLoopSettings();
    await refreshLoopGroups();
    state.ready = true;
  } catch (e) { state.error = String(e); }
  stopProfiles = watch(() => useAccountProfiles().profiles, () => { void saveLoopSettings().catch(e => { state.error = String(e); }); }, { deep: true });
  const poll = async () => {
    try { await refreshLoopGroups(); } catch (e) { state.error = String(e); }
    if (!closed) timer = setTimeout(poll, 2000);
  };
  timer = setTimeout(poll, 2000);
}
export function disposeLoopRouting() { closed = true; refreshGeneration++; clearTimeout(timer); stopProfiles?.(); }
export function useLoopRouting() {
  return { state, getByTab: (id: string | null) => state.groups.find(g => loopTabId(g.id) === id),
    async create(name: string, workspaceId: string, workspaceIndex: number, cwd: string, start: LoopStart, requestId?: string) {
      await requireExplicitStart();
      await saveLoopSettings();
      const group = await request<LoopGroup>({ op: 'create', name, workspaceId, workspaceIndex, cwd, start, requestId, cols: 120, rows: 30 });
      await refreshLoopGroups(group.id).catch(e => { state.error = String(e); });
      return group;
    },
    async conversations(cwd: string) {
      await requireExplicitStart();
      await saveLoopSettings();
      return request<LoopConversation[]>({ op: 'conversations', cwd });
    },
    async control(op: 'pause' | 'resume' | 'next' | 'stop', id: string) { await request({ op, id }); await refreshLoopGroups(); },
    async rename(id: string, name: string) {
      await request({ op: 'rename', id, name });
      await refreshLoopGroups();
    },
    async move(id: string, workspaceId: string, workspaceIndex: number) {
      await request({ op: 'move', id, workspaceId, workspaceIndex });
      const { state: workspaces, replaceLayout } = useWorkspaces();
      for (const ws of workspaces.workspaces) {
        if (ws.id !== workspaceId) replaceLayout(ws.id, removeTab(ws.layout, loopTabId(id)).root);
      }
      await refreshLoopGroups();
    },
    async close(id: string) {
      await request({ op: 'stop', id });
      const { state: workspaces, replaceLayout } = useWorkspaces();
      for (const ws of workspaces.workspaces) replaceLayout(ws.id, removeTab(ws.layout, loopTabId(id)).root);
      await refreshLoopGroups();
    },
    history: (id: string, attemptId: string) => request<string>({ op: 'history', id, attemptId }),
  };
}
