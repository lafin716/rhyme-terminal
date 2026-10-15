// Pure derivation for the left Navigator: given the Workspaces store and the
// daemon Session list, produce the grouped Workspace -> Session tree the panel
// renders, with the active Workspace and focused Session flagged.
//
// This module has no Vue or DOM dependency so it can be unit-tested directly
// (the pre-agreed test seam). Sessions map to their Workspace via the daemon
// name prefix `w{index}.`, using the shared `session-names` scheme helpers so
// Navigator labels match the Session tab labels exactly.

import type { Workspace } from "./layout-types";
import type { AgentKind, SessionInfo } from "./tauri";
import type { AgentTaskStatus } from "./tauri";
import { displayName, workspaceIndexOf } from "./session-names";

export interface NavigatorSessionNode {
  /** Daemon session id. */
  id: string;
  /** Full daemon name, including the Workspace prefix. */
  name: string;
  /** Un-prefixed label shown in the Navigator. */
  displayName: string;
  agent: AgentKind;
  isFocusedSession: boolean;
  /** Unseen output activity while unfocused — badged as a dot. */
  hasActivity: boolean;
  /** Rang a real bell — badged as a bell glyph, takes visual precedence. */
  hasBell: boolean;
  agentStatus?: AgentTaskStatus;
}

export interface NavigatorWorkspaceNode {
  id: string;
  name: string;
  icon?: string;
  index: number;
  isActiveWorkspace: boolean;
  sessions: NavigatorSessionNode[];
}

export interface NavigatorInput {
  workspaces: Pick<Workspace, "id" | "name" | "icon" | "index">[];
  sessions: Pick<SessionInfo, "id" | "name" | "agent">[];
  activeWorkspaceId: string | null;
  focusedSessionId: string | null;
  /**
   * Transient per-Session activity flags (the M4 `useSessions` map), keyed by
   * session id. Optional; sessions absent from it get no badge. The focused
   * Session is cleared upstream, so it naturally carries no flags here.
   */
  activityById?: Record<string, { output: boolean; bell: boolean }>;
  agentStatusById?: Record<string, AgentTaskStatus>;
}

export function buildNavigatorTree(input: NavigatorInput): NavigatorWorkspaceNode[] {
  const { workspaces, sessions, activeWorkspaceId, focusedSessionId } = input;
  const activityById = input.activityById ?? {};
  const agentStatusById = input.agentStatusById ?? {};

  // Bucket sessions by the Workspace index in their prefix. Sessions with no
  // prefix, or a prefix matching no Workspace, stay orphaned and are dropped.
  const byIndex = new Map<number, NavigatorSessionNode[]>();
  for (const s of sessions) {
    const index = workspaceIndexOf(s.name);
    if (index === null) continue;
    const activity = activityById[s.id];
    const node: NavigatorSessionNode = {
      id: s.id,
      name: s.name,
      displayName: displayName(s.name),
      agent: s.agent,
      isFocusedSession: s.id === focusedSessionId,
      hasActivity: activity?.output ?? false,
      hasBell: activity?.bell ?? false,
      agentStatus: agentStatusById[s.id],
    };
    const bucket = byIndex.get(index);
    if (bucket) bucket.push(node);
    else byIndex.set(index, [node]);
  }

  return workspaces.map((ws) => ({
    id: ws.id,
    name: ws.name,
    icon: ws.icon,
    index: ws.index,
    isActiveWorkspace: ws.id === activeWorkspaceId,
    sessions: byIndex.get(ws.index) ?? [],
  }));
}
