import type { Workspace } from './layout-types';
import type { SessionInfo } from './tauri';
import { collectAllSessionIds } from '../composables/useLayout';
import type { AccountProfile } from './persistence';

export type LoopAgent = 'claude' | 'codex';

export interface LoopCandidate {
  priority?: number;
  agent: LoopAgent;
  profileId: string | null;
  label: string;
  enabled: boolean;
  shortThreshold?: number | null;
  weeklyThreshold?: number | null;
  model?: string | null;
  effort?: string | null;
  mode?: string | null;
  configDir?: string;
  authMethod?: 'oauth' | 'setup-token';
  env?: Record<string, string>;
}

export interface LoopSettings {
  strategy?: 'SMART' | 'LEAST_USAGE' | 'ROUND_ROBIN' | 'PRIORITY';
  pollingIntervalSeconds?: number;
  interruptTimeoutSeconds?: number;
  forceKillTimeoutSeconds?: number;
  autoResume?: boolean;
  shortThreshold: number;
  weeklyThreshold: number;
  agentOrder: LoopAgent[];
  candidates: LoopCandidate[];
}

export interface LoopAttempt {
  id: string;
  sessionId: string | null;
  agent: LoopAgent;
  profileId: string | null;
  label: string;
  status: string;
  reason: string | null;
  startedAt: number;
  endedAt: number | null;
}

export interface AgentRuntimeState {
  status: 'IDLE' | 'STARTING' | 'RUNNING' | 'INTERRUPTING' | 'EXITED' | 'ERROR';
  provider: LoopAgent | null;
  pid: number | null;
  startedAt?: number | null;
}

export interface LoopProfileSnapshot {
  usagePending?: boolean;
  key: string;
  agent: LoopAgent;
  label: string;
  status:
    | 'ACTIVE'
    | 'AVAILABLE'
    | 'NEAR_LIMIT'
    | 'EXHAUSTED'
    | 'RATE_LIMITED'
    | 'WAITING_RESET'
    | 'DISABLED'
    | 'ERROR';
  usage: number | null;
  threshold: number;
  remaining: number | null;
  resetAt: number | null;
  error: string | null;
  windows?: { label: string; percentUsed: number; kind: string }[];
}

export type LoopPolicyPatch = Partial<
  Pick<LoopSettings, 'strategy' | 'pollingIntervalSeconds' | 'autoResume'>
> & {
  profile?: {
    key: string;
    shortThreshold?: number;
    weeklyThreshold?: number;
    priority?: number;
    model?: string | null;
    effort?: string | null;
    mode?: string | null;
  };
};

export interface LoopGroup {
  policy?: LoopSettings;
  queuedInput?: { id: string; bytes: number[]; delivering: boolean }[];
  runtime?: AgentRuntimeState;
  activeProfile?: string | null;
  profiles?: LoopProfileSnapshot[];
  currentProvider?: LoopAgent | null;
  currentAgentSessionId?: string | null;
  waitingProfileId?: string | null;
  resumeAt?: number | null;
  events?: { at: number; kind: string; message: string }[];
  id: string;
  name: string;
  workspaceId: string;
  cwd: string;
  status:
    | 'idle'
    | 'preparing'
    | 'running'
    | 'switching_profile'
    | 'handoff'
    | 'waiting_for_usage_reset'
    | 'resuming'
    | 'paused'
    | 'stopped'
    | 'error'
    | 'recovery';
  activeSessionId: string | null;
  reason: string | null;
  attempts: LoopAttempt[];
  updatedAt: number;
  switchPending?: boolean;
  switchPendingSince?: number | null;
}

export const loopTabId = (id: string) => `loop:${id}`;

export function mergeLoopProfiles(
  settings: LoopSettings,
  profiles: AccountProfile[],
): LoopSettings {
  const available: LoopCandidate[] = profiles.map(p => ({
    agent: p.agent,
    profileId: p.id,
    label: p.label,
    enabled: false,
    configDir: p.configDir,
    authMethod: p.authMethod === 'setup-token' ? 'setup-token' : 'oauth',
    env: p.env,
  }));
  available.push(
    ...(['claude', 'codex'] as const).map(agent => ({
      agent,
      profileId: null,
      label: '시스템 기본',
      enabled: false,
    })),
  );

  const key = (p: LoopCandidate) => `${p.agent}:${p.profileId ?? 'system'}`;
  const ordered = settings.candidates.flatMap(old => {
    const p = available.find(candidate => key(candidate) === key(old));
    return p
      ? [
          {
            ...p,
            enabled: old.enabled,
            priority: old.priority ?? 0,
            shortThreshold: old.shortThreshold,
            weeklyThreshold: old.weeklyThreshold,
            model: old.model,
            effort: old.effort,
            mode: old.mode,
          },
        ]
      : [];
  });
  return {
    ...settings,
    candidates: [
      ...ordered,
      ...available.filter(p => !ordered.some(o => key(o) === key(p))),
    ],
  };
}

export const loopStatusLabel = (status: string) =>
  ({
    idle: '대기',
    preparing: '초기화 중',
    running: '실행 중',
    switching_profile: '교체 대기',
    handoff: '위임 대기',
    waiting_for_usage_reset: 'Usage Reset 대기',
    resuming: '재개 대기',
    paused: '일시중지',
    stopped: '종료됨',
    error: '오류',
    recovery: '복구 중',
  }[status] ?? status);

/**
 * Every PTY an Agent Loop owns: the shell it runs in now plus each historical
 * attempt. None of them may sit in a layout as an ordinary terminal. The
 * Loop's own `loopTabId` tab already represents it; the daemon respawns the
 * shell whenever it is missing and the group is not stopped, so such a tab
 * cannot be closed; and after a restart the shell returns under a new id,
 * leaving any tab persisted for the old one stale.
 */
export function loopManagedSessionIds(
  groups: Pick<LoopGroup, 'activeSessionId' | 'attempts'>[],
): Set<string> {
  return new Set(
    groups
      .flatMap(group => [group.activeSessionId, ...group.attempts.map(attempt => attempt.sessionId)])
      .filter((id): id is string => id !== null),
  );
}

export function loopNavigatorSessions(
  sessions: Pick<SessionInfo, 'id' | 'name' | 'agent'>[],
  groups: LoopGroup[],
  workspaces: Pick<Workspace, 'id' | 'index' | 'layout'>[],
): Pick<SessionInfo, 'id' | 'name' | 'agent'>[] {
  const managedIds = loopManagedSessionIds(groups);
  const result = sessions.filter(session => !managedIds.has(session.id));
  for (const group of groups) {
    const ws = workspaces.find(ws => ws.id === group.workspaceId);
    if (!ws || !collectAllSessionIds(ws.layout).includes(loopTabId(group.id))) continue;
    result.push({
      id: loopTabId(group.id),
      name: `w${ws.index}.${group.name}`,
      agent: group.runtime?.provider ?? 'terminal',
    });
  }
  return result;
}
