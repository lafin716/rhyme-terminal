export interface SessionInfo {
  id: string;
  name: string;
  shell: string;
  cwd: string | null;
  cols: number;
  rows: number;
  agent?: string;
}

export interface AttachResult {
  info: SessionInfo;
  scrollback: string;
}

export type ConnectionPhase =
  | "connecting"
  | "pending"
  | "authenticated"
  | "reconnecting"
  | "disconnected"
  | "error";

export interface ClientState {
  phase: ConnectionPhase;
  verification: string | null;
  error: string | null;
  sessions: SessionInfo[];
  attached: SessionInfo | null;
}
