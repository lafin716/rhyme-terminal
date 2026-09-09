import { describe, expect, it } from 'vitest';
import { makeLeaf } from './layout-types';
import { loopNavigatorSessions, type LoopGroup, loopTabId, mergeLoopProfiles, type LoopSettings } from './loop-routing';
const settings: LoopSettings = { shortThreshold: 90, weeklyThreshold: 90, agentOrder: ['claude', 'codex'], candidates: [] };
const profile = (id: string) => ({ id, agent: 'claude' as const, label: id, configDir: `C:/profiles/${id}`, createdAt: 0, authMethod: 'oauth' as const });
describe('loop routing profile membership', () => {
  it('requires explicit opt in for all new profiles and system accounts', () => {
    const merged = mergeLoopProfiles(settings, [profile('a')]);
    expect(merged.candidates).toHaveLength(3);
    expect(merged.candidates.every(p => !p.enabled)).toBe(true);
  });
  it('preserves candidate order and overrides but refreshes profile metadata and excludes newly added profiles', () => {
    const merged = mergeLoopProfiles({ ...settings, candidates: [{ agent: 'claude', profileId: 'b', label: 'old', enabled: true, shortThreshold: 80 }, { agent: 'claude', profileId: 'a', label: 'a', enabled: true }] }, [profile('a'), profile('b'), profile('c')]);
    expect(merged.candidates.map(p => p.profileId)).toEqual(['b', 'a', 'c', null, null]);
    expect(merged.candidates[0]).toMatchObject({ label: 'b', shortThreshold: 80, enabled: true });
    expect(merged.candidates[2].enabled).toBe(false);
  });
  it('removes deleted profiles and keeps stable group IDs separate from PTY IDs', () => {
    const merged = mergeLoopProfiles({ ...settings, candidates: [{ agent: 'claude', profileId: 'deleted', label: 'gone', enabled: true }] }, []);
    expect(merged.candidates.some(p => p.profileId === 'deleted')).toBe(false);
    expect(loopTabId('abc')).toBe('loop:abc');
  });
});

describe('loop navigator rows', () => {
  const group: LoopGroup = { id: 'g1', name: 'Loop', workspaceId: 'w2', cwd: 'C:/repo', status: 'running', activeSessionId: 'live', attempts: [{ id: 'a1', sessionId: 'old', agent: 'codex', profileId: 'p1', label: 'Work', status: 'completed', reason: null, startedAt: 0, endedAt: 1 }], reason: null, updatedAt: 1 };
  it('hides all managed PTYs and shows the group once in its current workspace namespace', () => {
    const sessions = [{ id: 'ordinary', name: 'w1.shell', agent: 'terminal' as const }, { id: 'old', name: 'w1.old', agent: 'claude' as const }, { id: 'live', name: 'w1.current', agent: 'codex' as const }];
    expect(loopNavigatorSessions(sessions, [group], [{ id: 'w2', index: 2, layout: makeLeaf('leaf', ['loop:g1']) }])).toEqual([sessions[0], { id: 'loop:g1', name: 'w2.Loop', agent: 'codex' }]);
  });
  it('does not resurrect a closed group as a navigator row', () => {
    expect(loopNavigatorSessions([], [{ ...group, status: 'stopped' }], [{ id: 'w2', index: 2, layout: makeLeaf('leaf') }])).toEqual([]);
  });
});