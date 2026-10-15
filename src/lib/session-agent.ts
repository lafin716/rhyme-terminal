import type { AgentKind, SessionAgentPayload } from "./tauri";

export interface VersionedSessionAgentPayload {
  revision: number;
  payload: SessionAgentPayload;
}

export function applySessionAgent<T extends { id: string; agent: AgentKind }>(
  sessions: T[],
  update: SessionAgentPayload,
): T[] {
  const index = sessions.findIndex((session) => session.id === update.id);
  if (index < 0 || sessions[index].agent === update.agent) return sessions;

  const next = sessions.slice();
  next[index] = { ...sessions[index], agent: update.agent };
  return next;
}

/**
 * Applies only the agent events that arrived after an async session snapshot
 * request began. That keeps a late list/create response from overwriting a
 * newer event while allowing a newer daemon snapshot to replace older events.
 */
export function applySessionAgentUpdates<T extends { id: string; agent: AgentKind }>(
  sessions: T[],
  updates: Iterable<VersionedSessionAgentPayload>,
  snapshotRevision: number,
): T[] {
  let next = sessions;
  for (const update of updates) {
    if (update.revision > snapshotRevision) {
      next = applySessionAgent(next, update.payload);
    }
  }
  return next;
}
