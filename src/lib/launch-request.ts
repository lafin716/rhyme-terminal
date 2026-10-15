// A launch request as the backend hands it over, and the pure rule for turning
// one into the options a session is created with.
//
// Every external integration — Explorer's context menus, the command line, an
// external tool configured to use Rhyme Terminal, a `rhyme://` link — arrives
// here in this one shape (see `src-tauri/src/launch/mod.rs`), so this file is
// the whole of what the UI has to know about any of them.

/** Which integration spelled the request. Diagnostics, not behaviour. */
export type LaunchSource = "cli" | "explorer" | "uri";

export interface LaunchRequest {
  cwd?: string | null;
  files?: string[];
  command?: string | null;
  ssh?: string | null;
  newWindow?: boolean;
  source?: LaunchSource;
}

/** What `useSessions().create()` is called with for one request. */
export interface LaunchSessionOptions {
  cwd?: string;
  /** Spliced into the new shell's startup args — see `withAgentLaunch`. */
  launchCommand?: string;
  /**
   * Paths the request named, to offer at the prompt once the shell is up.
   * Never executed: a path is typed, exactly as for a drop.
   */
  paths: string[];
}

/**
 * The session one request asks for.
 *
 * `command` wins over `ssh` when both are set: a caller that spelled out a
 * command was more specific than one that only named a host. An `ssh` target
 * becomes `ssh <target>` here rather than in the backend, so how this app
 * reaches a host stays a decision of the app's and not of its callers'.
 */
export function launchSessionOptions(request: LaunchRequest): LaunchSessionOptions {
  const command = request.command?.trim();
  const ssh = request.ssh?.trim();
  return {
    ...(request.cwd ? { cwd: request.cwd } : {}),
    ...(command ? { launchCommand: command } : ssh ? { launchCommand: `ssh ${ssh}` } : {}),
    paths: request.files ?? [],
  };
}

/** Whether a request asks for anything beyond raising the window. */
export function isActionableLaunch(request: LaunchRequest): boolean {
  return Boolean(
    request.cwd || request.command?.trim() || request.ssh?.trim() || request.files?.length,
  );
}
