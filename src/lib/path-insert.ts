// Pure helpers for inserting filesystem paths into a terminal's input line.
//
// A path dropped on a Terminal is *typed*, not executed: it is written to the
// PTY exactly as a user would type it, and no newline is ever appended (see
// `Terminal.vue`). What "as a user would type it" means differs per shell, so
// the quoting lives here behind one entry point and is unit-tested per shell —
// the project's pure test-seam style, like `quick-open.ts`.
//
// No Vue/DOM/Tauri dependency.

import type { TerminalPreset } from "./terminal-config";

// dataTransfer MIME type carrying an Explorer file drag. Distinct from the
// Pane-tab drag (which uses `text/plain` = sessionId), so the two gestures
// never interfere. Shared here so the Explorer (drag source) and Terminal
// (drop target) can't drift apart on a typo.
//
// The payload is one path per line: a single drag carries one, a multi-select
// carries several, and a receiver that splits on newlines handles both.
export const FILE_DRAG_MIME = "application/x-winmux-file";

/** Which quoting rules a shell follows. Several presets share one family. */
export type ShellQuoting = "powershell" | "cmd" | "posix";

/**
 * The quoting family a terminal preset belongs to.
 *
 * `custom` is the open question: the program is whatever the user configured,
 * so this guesses from the executable name and falls back to PowerShell, the
 * Windows default and the one whose quoting is safe to read literally in cmd
 * as well (both understand a quoted argument; only the escape character for a
 * quote *inside* differs, and Windows paths cannot contain one).
 */
export function shellQuoting(preset: TerminalPreset, program = ""): ShellQuoting {
  switch (preset) {
    case "windows-powershell":
    case "powershell":
      return "powershell";
    case "cmd":
      return "cmd";
    case "wsl":
    case "git-bash":
    case "zsh":
      return "posix";
    case "custom":
    default: {
      const name = program.toLowerCase().replace(/\\/g, "/").split("/").pop() ?? "";
      if (name.startsWith("cmd")) return "cmd";
      if (name.startsWith("pwsh") || name.startsWith("powershell")) return "powershell";
      if (/^(ba|z|k|fi|da|)sh(\.exe)?$|^wsl|^bash/.test(name)) return "posix";
      return "powershell";
    }
  }
}

/**
 * A Windows path as the shell on the other side of the PTY can resolve it.
 *
 * WSL runs in its own filesystem namespace, where a Windows drive is mounted
 * under `/mnt`, so `C:\x` has to become `/mnt/c/x` — pasting the Windows form
 * there produces a path that simply does not exist. Git Bash (MSYS) resolves
 * `C:/x` but not `C:\x`, because it reads the backslash as an escape, so only
 * the separators change and the drive letter stays. Everything else is left
 * exactly as Explorer spelled it.
 */
export function pathForShell(path: string, preset: TerminalPreset): string {
  if (preset === "wsl") {
    const drive = /^([A-Za-z]):[\\/](.*)$/.exec(path);
    if (drive) return `/mnt/${drive[1].toLowerCase()}/${drive[2].replace(/\\/g, "/")}`;
    return path.replace(/\\/g, "/");
  }
  if (preset === "git-bash") return path.replace(/\\/g, "/");
  return path;
}

/**
 * Quote one already-shell-shaped path so it survives as a single argument.
 *
 * PowerShell and POSIX shells both take a single-quoted string literally,
 * which is what a path wants: no variable expansion, no escape processing, so
 * `$`, backticks and backslashes need no further care. They differ only in how
 * a literal `'` is written — doubled for PowerShell, closed-escaped-reopened
 * for POSIX. cmd has no literal-quote form at all, but it also cannot receive
 * a path containing `"`, since Windows forbids that character in a filename.
 */
export function quoteForShell(path: string, quoting: ShellQuoting): string {
  if (!path) return "";
  switch (quoting) {
    case "powershell":
      return `'${path.replace(/'/g, "''")}'`;
    case "posix":
      return `'${path.replace(/'/g, "'\\''")}'`;
    case "cmd":
      // `"` cannot occur in a Windows path, so quoting always terminates.
      return `"${path.replace(/"/g, "")}"`;
  }
}

/**
 * The text to type at the prompt for one dropped path.
 *
 * Always quoted, even when nothing in the path needs it: an unquoted path is
 * one rename away from breaking, and the quotes are what make the result safe
 * to read at a glance.
 */
export function formatPathForShell(path: string, preset: TerminalPreset, program = ""): string {
  if (!path) return "";
  return quoteForShell(pathForShell(path, preset), shellQuoting(preset, program));
}

/**
 * The text to type for a whole drop: every path, space-separated, in the order
 * they were dragged. Empty entries are dropped so a trailing newline in the
 * payload cannot produce a stray `''`.
 */
export function formatPathsForShell(
  paths: readonly string[],
  preset: TerminalPreset,
  program = "",
): string {
  return paths
    .map((path) => formatPathForShell(path.trim(), preset, program))
    .filter(Boolean)
    .join(" ");
}

/** Paths carried by one drag payload — one per line. */
export function parseDragPayload(payload: string): string[] {
  return payload
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

/** A drag payload for the given paths. Inverse of {@link parseDragPayload}. */
export function toDragPayload(paths: readonly string[]): string {
  return paths.filter(Boolean).join("\n");
}

/**
 * Format a filesystem path for insertion at a terminal's cursor, without
 * knowing which shell is on the other side.
 *
 * Quotes the path when it contains whitespace so the shell treats it as one
 * argument; otherwise returns it unchanged. Kept for callers that have no
 * session context — prefer {@link formatPathForShell} where the preset is
 * known, since it also fixes up WSL and Git Bash paths.
 */
export function formatPathForInsertion(path: string): string {
  if (!path) return "";
  if (/\s/.test(path)) return `"${path}"`;
  return path;
}
