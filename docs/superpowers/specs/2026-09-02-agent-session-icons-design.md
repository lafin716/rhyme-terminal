# Agent session icons design

## Goal

Show a Claude icon for a terminal actively running Claude Code, a Codex icon
for a terminal actively running Codex, and the existing terminal icon for all
other sessions. The workspace Navigator and terminal tabs must use the same
state and icon selection.

## Process detection

Each PTY Session retains the PID of the shell process created by
`portable-pty`. The daemon periodically reads the Windows process table and
walks descendants of that root PID. A descendant is classified from its
executable filename and command line:

- `claude` or `claude.exe` is `claude`
- `codex` or `codex.exe` is `codex`
- no matching descendant is `terminal`

If both are present, the most recently started matching descendant wins. The
state falls back to `terminal` when the agent exits. Process-query failure also
falls back safely to `terminal`.

## Daemon and IPC

`SessionInfo` gains an `agent` field with `terminal`, `claude`, or `codex`.
The daemon owns the monitor and publishes a `SessionAgentChanged` event only
when a session's derived agent changes. Existing `list_sessions` responses
include the current agent, so startup and reconnect remain correct without
waiting for an event.

The Tauri bridge exposes the field and listens for the update event. The
shared sessions store applies updates in place, avoiding a polling loop in the
webview.

## UI

A dependency-free icon resolver maps the session's agent state to three local
SVG icons: Claude, Codex, or the existing generic terminal glyph. Both
`SideBar.vue` and `PaneTabs.vue` call this resolver. File and browser tab
icons remain unchanged.

## Verification

- Unit-test process-name/command-line classification and descendant selection.
- Unit-test frontend agent-to-icon resolution.
- Build TypeScript and run the existing frontend tests.
- Run Rust tests for the daemon and IPC types.
