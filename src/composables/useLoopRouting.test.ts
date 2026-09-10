import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLeaf } from '../lib/layout-types';
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), refresh: vi.fn(), replaceLayout: vi.fn(), profiles: [] as unknown[], workspaces: [] as unknown[] }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }));
vi.mock('./useSessions', () => ({ useSessions: () => ({ refresh: mocks.refresh }) }));
vi.mock('./usePrefs', () => ({ usePrefs: () => ({ prefs: { systemAccountEnv: { claude: { SYSTEM_SETTING: 'system-value' }, codex: {} } } }) }));
vi.mock('./useAccountProfiles', () => ({ useAccountProfiles: () => ({ profiles: mocks.profiles }) }));
vi.mock('./useWorkspaces', () => ({ useWorkspaces: () => ({ state: { workspaces: mocks.workspaces }, replaceLayout: mocks.replaceLayout }) }));
import { refreshLoopGroups, saveLoopSettings, useLoopRouting } from './useLoopRouting';
const group = (activeSessionId: string | null) => ({ id: 'group-1', name: 'Loop', workspaceId: 'w1', cwd: 'C:/repo', status: 'running', activeSessionId, reason: null, attempts: [], updatedAt: 0 });
beforeEach(() => {
  mocks.invoke.mockReset();
  mocks.refresh.mockClear();
  mocks.profiles.length = 0;
  mocks.workspaces.length = 0;
  vi.stubGlobal('localStorage', { setItem: vi.fn(), getItem: vi.fn() });
});
describe('loop tab synchronization', () => {
  it('ignores an older poll that arrives after a group was stopped', async () => {
    const leaf = makeLeaf('leaf');
    mocks.workspaces.push({ id: 'w1', layout: leaf });
    let release!: (value: unknown) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(resolve => { release = resolve; }))
      .mockResolvedValueOnce([{ ...group(null), status: 'stopped' }]);
    const earlier = refreshLoopGroups();
    await refreshLoopGroups();
    release([group('old-pty')]);
    await earlier;
    expect(useLoopRouting().state.groups[0].status).toBe('stopped');
    expect(leaf.tabs).toEqual([]);
  });
  it('preserves stable tab identity and selected ordinary tab when the active PTY changes', async () => {
    const leaf = makeLeaf('leaf', ['ordinary']);
    leaf.activeTabId = 'ordinary';
    mocks.workspaces.push({ id: 'w1', layout: leaf });
    mocks.invoke.mockResolvedValue([group('pty-a')]);
    await refreshLoopGroups();
    mocks.invoke.mockResolvedValue([group('pty-b')]);
    await refreshLoopGroups();
    expect(leaf.tabs).toEqual(['ordinary', 'loop:group-1']);
    expect(leaf.activeTabId).toBe('ordinary');
    expect(useLoopRouting().getByTab('loop:group-1')?.activeSessionId).toBe('pty-b');
    expect(mocks.refresh).toHaveBeenCalledTimes(2);
    await refreshLoopGroups();
    expect(mocks.refresh).toHaveBeenCalledTimes(2);
    mocks.invoke.mockResolvedValue([group(null)]);
    await refreshLoopGroups();
    expect(mocks.refresh).toHaveBeenCalledTimes(3);
    await refreshLoopGroups();
    expect(mocks.refresh).toHaveBeenCalledTimes(3);
  });
  it('does not reopen stopped groups or insert a freshly created group in the wrong pane', async () => {
    const leaf = makeLeaf('leaf');
    mocks.workspaces.push({ id: 'w1', layout: leaf });
    mocks.invoke.mockResolvedValue([group('pty-a')]);
    await refreshLoopGroups('group-1');
    expect(leaf.tabs).toEqual([]);
    mocks.invoke.mockResolvedValue([{ ...group('pty-a'), status: 'stopped' }]);
    await refreshLoopGroups();
    expect(leaf.tabs).toEqual([]);
  });
  it('keeps local profile environment when the backend redacts configure responses', async () => {
    mocks.profiles.push({ id: 'account', agent: 'codex', label: 'Work', configDir: 'C:/profile', env: { EXAMPLE_SETTING: 'value' } });
    mocks.invoke.mockResolvedValue({});
    await saveLoopSettings({ shortThreshold: 90, weeklyThreshold: 90, agentOrder: ['claude', 'codex'], candidates: [] });
    expect(useLoopRouting().state.settings.candidates[0].env).toEqual({ EXAMPLE_SETTING: 'value' });
    const persisted = JSON.parse(vi.mocked(localStorage.setItem).mock.calls[0][1]);
    expect(persisted.candidates[0].env).toBeUndefined();
    expect(persisted.candidates.every((p: { env?: unknown }) => p.env === undefined)).toBe(true);
    expect(useLoopRouting().state.settings.candidates.find(p => p.agent === 'claude' && p.profileId === null)?.env).toEqual({ SYSTEM_SETTING: 'system-value' });
  });
});

describe('loop workspace ownership', () => {
  it('moves the backend group before transferring its stable tab and closes it without killing a synthetic PTY', async () => {
    const source = makeLeaf('source', ['ordinary', 'loop:group-1']);
    const target = makeLeaf('target');
    mocks.workspaces.push({ id: 'w1', layout: source }, { id: 'w2', layout: target });
    mocks.invoke.mockImplementation(async (_command, args) => args.request.op === 'list' ? [{ ...group('pty-a'), workspaceId: 'w2' }] : null);
    await useLoopRouting().move('group-1', 'w2', 2);
    expect(mocks.invoke).toHaveBeenCalledWith('loop_request', { request: { op: 'move', id: 'group-1', workspaceId: 'w2', workspaceIndex: 2 } });
    expect(source.tabs).toEqual(['ordinary']);
    expect(target.tabs).toEqual(['loop:group-1']);
    mocks.invoke.mockImplementation(async (_command, args) => args.request.op === 'list' ? [{ ...group('pty-a'), workspaceId: 'w2', status: 'stopped' }] : null);
    await useLoopRouting().close('group-1');
    expect(mocks.invoke).toHaveBeenCalledWith('loop_request', { request: { op: 'stop', id: 'group-1' } });
    expect(target.tabs).toEqual([]);
    expect(mocks.invoke.mock.calls.every(call => call[0] === 'loop_request')).toBe(true);
  });
});


it('renames a loop through its real group id without changing the stable tab', async () => {
  mocks.invoke.mockImplementation(async (_command,args)=>args.request.op==='list'?[{...group('pty-a'),name:'Renamed loop'}]:null);
  await useLoopRouting().rename('group-1','Renamed loop');
  expect(mocks.invoke).toHaveBeenCalledWith('loop_request',{request:{op:'rename',id:'group-1',name:'Renamed loop'}});
  expect(useLoopRouting().getByTab('loop:group-1')?.name).toBe('Renamed loop');
});

it('requests guarded automatic creation without a prompt or session selector', async () => {
  mocks.invoke.mockImplementation(async (_command, args) => args.request.op === 'create' ? { ...group('shell'), status: 'resuming' } : args.request.op === 'list' ? [] : { runtimeMonitor: true, autoStart: true });
  await useLoopRouting().create({ name: 'Checkout', workspaceId: 'w1', workspaceIndex: 1, cwd: 'C:/repo', participants: ['codex:work'] });
  expect(mocks.invoke).toHaveBeenCalledWith('loop_request', { request: expect.objectContaining({ op: 'create', participants: ['codex:work'] }) });
  const request = mocks.invoke.mock.calls.find(([, args]) => args.request.op === 'create')![1].request;
  expect(request).not.toHaveProperty('start');
  expect(mocks.invoke.mock.calls.some(([, args]) => args.request.op === 'conversations')).toBe(false);
});
it('blocks a runtime-aware daemon without automatic startup before creation', async () => {
  mocks.invoke.mockResolvedValue({ version: 1, runtimeMonitor: true });
  await expect(useLoopRouting().create({ name: 'New', workspaceId: 'w1', workspaceIndex: 1, cwd: 'C:/repo' })).rejects.toThrow('데몬');
  expect(mocks.invoke.mock.calls.every(([, args]) => args.request.op === 'capabilities')).toBe(true);
});

it('updates only the requested loop policy and refreshes its persisted state', async () => {
  mocks.invoke.mockImplementation(async (_command, args) => args.request.op === 'capabilities' ? { livePolicy: true } : args.request.op === 'list' ? [group('shell')] : group('shell'));
  await useLoopRouting().updatePolicy('group-1', { profile: { key: 'codex:work', shortThreshold: 75 } });
  expect(mocks.invoke).toHaveBeenCalledWith('loop_request', { request: { op: 'update_policy', id: 'group-1', patch: { profile: { key: 'codex:work', shortThreshold: 75 } } } });
  expect(mocks.invoke.mock.calls.some(([, args]) => args.request.op === 'configure')).toBe(false);
});
it('rejects live editing on an older daemon without writing settings', async () => {
  mocks.invoke.mockResolvedValue({ runtimeMonitor: true, autoStart: true });
  await expect(useLoopRouting().updatePolicy('group-1', { autoResume: false })).rejects.toThrow('데몬');
  expect(mocks.invoke.mock.calls.every(([, args]) => args.request.op === 'capabilities')).toBe(true);
});
