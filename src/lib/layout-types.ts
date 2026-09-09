import type { TerminalConfig } from "./terminal-config";

export type Direction = "horizontal" | "vertical";

export interface LeafNode {
  kind: "leaf";
  id: string;
  tabs: string[];
  activeTabId: string | null;
}

export interface SplitNode {
  kind: "split";
  id: string;
  direction: Direction;
  sizes: number[];
  children: LayoutNode[];
}

export type LayoutNode = LeafNode | SplitNode;

export interface WorkspaceSettings {
  // Optional default working directory for new sessions in this workspace.
  defaultCwd: string;
  // Null inherits the global terminal preference.
  terminal: TerminalConfig | null;
  // Pinned root of the right Explorer's file tree for this workspace. When
  // unset the Explorer falls back to defaultCwd, then the focused session's cwd
  // on first open. Persisted so each workspace remembers its own root.
  explorerRoot?: string;
}

export interface TerminalTabSnapshot {
  agent?: import("./tauri").AgentKind;
  accountProfile?: import("./session-profile").SessionProfile;
  name: string;
  terminal: TerminalConfig;
  cwd?: string | null;
}

export interface Workspace {
  id: string;
  name: string;
  icon?: string;
  // 1-based, monotonic per session-store; never reused. Used as the daemon-name prefix
  // so session names remain unique across workspaces (e.g. `w1.session-1`).
  index: number;
  // Deprecated — session-number allocation is now derived from live session names.
  // Field is retained for back-compat with persisted stores.
  nextSessionSeq: number;
  settings: WorkspaceSettings;
  layout: LayoutNode;
  terminalSnapshots: Record<string, TerminalTabSnapshot>;
  sessionOrder?: string[];
}

export interface WorkspaceStore {
  workspaces: Workspace[];
  activeWorkspaceId: string;
}

export type DropZone = "center" | "left" | "right" | "top" | "bottom";

export function makeLeaf(id: string, tabs: string[] = [], activeTabId: string | null = null): LeafNode {
  return { kind: "leaf", id, tabs, activeTabId };
}

export function nodeId(prefix: string): string {
  return `${prefix}-${Math.random().toString(36).slice(2, 10)}`;
}
