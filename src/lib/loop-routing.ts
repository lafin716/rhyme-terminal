import type { Workspace } from './layout-types';
import type { SessionInfo } from './tauri';
import { collectAllSessionIds } from '../composables/useLayout';
import type { AccountProfile } from './persistence';
export type LoopAgent = 'claude' | 'codex';
export interface LoopCandidate { agent: LoopAgent; profileId: string | null; label: string; enabled: boolean; shortThreshold?: number | null; weeklyThreshold?: number | null; configDir?: string; authMethod?: 'oauth' | 'setup-token'; env?: Record<string, string> }
export interface LoopSettings { shortThreshold: number; weeklyThreshold: number; agentOrder: LoopAgent[]; candidates: LoopCandidate[] }
export interface LoopAttempt { id: string; sessionId: string | null; agent: LoopAgent; profileId: string | null; label: string; status: string; reason: string | null; startedAt: number; endedAt: number | null }
export interface LoopGroup { id: string; name: string; workspaceId: string; cwd: string; status: 'starting' | 'running' | 'switch_pending' | 'switching' | 'waiting' | 'paused' | 'recovery' | 'stopped'; activeSessionId: string | null; reason: string | null; attempts: LoopAttempt[]; updatedAt: number }
export const loopTabId = (id: string) => `loop:${id}`;
export function mergeLoopProfiles(settings: LoopSettings, profiles: AccountProfile[]): LoopSettings {
  const available: LoopCandidate[] = profiles.map(p => ({ agent: p.agent, profileId: p.id, label: p.label, enabled: false, configDir: p.configDir, authMethod: p.authMethod === 'setup-token' ? 'setup-token' : 'oauth', env: p.env }));
  available.push(...(['claude', 'codex'] as const).map(agent => ({ agent, profileId: null, label: '시스템 계정', enabled: false })));
  const key = (p: LoopCandidate) => `${p.agent}:${p.profileId ?? 'system'}`;
  const ordered = settings.candidates.flatMap(old => {
    const p = available.find(p => key(p) === key(old));
    return p ? [{ ...p, enabled: old.enabled, shortThreshold: old.shortThreshold, weeklyThreshold: old.weeklyThreshold }] : [];
  });
  return { ...settings, candidates: [...ordered, ...available.filter(p => !ordered.some(o => key(o) === key(p)))] };
}
export const loopStatusLabel = (status: string) => ({starting:'시작 중',running:'실행 중',switch_pending:'전환 대기',switching:'전환 중',waiting:'계정 대기',paused:'일시정지',recovery:'복구 필요',stopped:'중지됨'}[status] ?? status);

/** Show one stable group row, never its underlying current or historical PTYs. */
export function loopNavigatorSessions(sessions: Pick<SessionInfo, 'id' | 'name' | 'agent'>[], groups: LoopGroup[], workspaces: Pick<Workspace, 'id' | 'index' | 'layout'>[]): Pick<SessionInfo, 'id' | 'name' | 'agent'>[] {
  const managedIds = new Set(groups.flatMap(group => [group.activeSessionId, ...group.attempts.map(attempt => attempt.sessionId)].filter((id): id is string => id !== null)));
  const result = sessions.filter(session => !managedIds.has(session.id));
  for (const group of groups) {
    const ws = workspaces.find(ws => ws.id === group.workspaceId);
    if (!ws || !collectAllSessionIds(ws.layout).includes(loopTabId(group.id))) continue;
    result.push({ id: loopTabId(group.id), name: `w${ws.index}.${group.name}`, agent: group.attempts.slice(-1)[0]?.agent ?? 'terminal' });
  }
  return result;
}
export type LoopStart = { kind: 'prompt'; prompt: string } | { kind: 'session'; sessionId: string; candidateKey: string };
export interface LoopConversation { sessionId: string; candidateKey: string; agent: LoopAgent; label: string; title: string; updatedAt: number }
