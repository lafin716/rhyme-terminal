import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AgentKind = "terminal" | "claude" | "codex";
export type AgentTaskStatus = "working" | "completed" | "error";

export interface SessionInfo {
  id: string;
  name: string;
  shell: string;
  cwd?: string | null;
  cols: number;
  rows: number;
  agent: AgentKind;
}

export interface FilePreview {
  canonicalPath: string;
  name: string;
  kind: "text" | "image" | "binary" | "too_large" | "pdf";
  language: string;
  mime: string;
  text?: string | null;
  data?: string | null;
  size: number;
  line?: number | null;
  column?: number | null;
}

export interface DirEntry {
  name: string;
  path: string;
  isDir: boolean;
  hidden: boolean;
}

export interface DirListing {
  path: string;
  entries: DirEntry[];
}

export interface FileEntry {
  path: string; // absolute
  relPath: string; // root-relative display path, always `/`-joined
}

export interface FileIndex {
  root: string;
  files: FileEntry[];
}

export interface PtyOutputPayload {
  id: string;
  data: string; // base64
}

export interface PtyExitPayload {
  id: string;
}

export interface SessionActivityPayload {
  id: string;
  bell: boolean;
}

export interface SessionAgentPayload {
  id: string;
  agent: AgentKind;
}

export interface SessionAgentStatusPayload {
  id: string;
  status: AgentTaskStatus;
}

export const api = {
  createSession(opts: {
    name?: string;
    shell?: string;
    shellArgs?: string[];
    cwd?: string;
    cols?: number;
    rows?: number;
  } = {}): Promise<SessionInfo> {
    return invoke("create_session", opts);
  },
  listSessions(): Promise<SessionInfo[]> {
    return invoke("list_sessions");
  },
  killSession(id: string): Promise<void> {
    return invoke("kill_session", { id });
  },
  writeSession(id: string, data: string): Promise<void> {
    // data must be base64
    return invoke("write_session", { id, data });
  },
  resizeSession(id: string, cols: number, rows: number): Promise<void> {
    return invoke("resize_session", { id, cols, rows });
  },
  attachSession(id: string): Promise<string> {
    return invoke("attach_session", { id });
  },
  renameSession(id: string, name: string): Promise<void> {
    return invoke("rename_session", { id, name });
  },
  readFilePreview(target: string, cwd?: string): Promise<FilePreview> {
    return invoke("read_file_preview", { target, cwd });
  },
  writeFile(path: string, contents: string): Promise<void> {
    return invoke("write_file", { path, contents });
  },
  readDirectory(path: string): Promise<DirListing> {
    return invoke("read_directory", { path });
  },
  listFiles(root: string): Promise<FileIndex> {
    return invoke("list_files", { root });
  },
  browserNavigate(label: string, url: string): Promise<void> {
    return invoke("browser_navigate", { label, url });
  },
  browserBack(label: string): Promise<void> {
    return invoke("browser_back", { label });
  },
  browserForward(label: string): Promise<void> {
    return invoke("browser_forward", { label });
  },
  browserReload(label: string): Promise<void> {
    return invoke("browser_reload", { label });
  },
};

export function onPtyOutput(
  handler: (payload: PtyOutputPayload) => void,
): Promise<UnlistenFn> {
  return listen<PtyOutputPayload>("pty-output", (e) => handler(e.payload));
}

export function onPtyExit(
  handler: (payload: PtyExitPayload) => void,
): Promise<UnlistenFn> {
  return listen<PtyExitPayload>("pty-exit", (e) => handler(e.payload));
}

export function onSessionActivity(
  handler: (payload: SessionActivityPayload) => void,
): Promise<UnlistenFn> {
  return listen<SessionActivityPayload>("session-activity", (e) => handler(e.payload));
}

export function onSessionAgentChanged(
  handler: (payload: SessionAgentPayload) => void,
): Promise<UnlistenFn> {
  return listen<SessionAgentPayload>("session-agent-changed", (e) => handler(e.payload));
}

export function onSessionAgentStatusChanged(
  handler: (payload: SessionAgentStatusPayload) => void,
): Promise<UnlistenFn> {
  return listen<SessionAgentStatusPayload>("session-agent-status-changed", (e) => handler(e.payload));
}

// Helpers for base64 <-> binary
export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

export function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}

export function stringToBase64(str: string): string {
  return bytesToBase64(new TextEncoder().encode(str));
}
