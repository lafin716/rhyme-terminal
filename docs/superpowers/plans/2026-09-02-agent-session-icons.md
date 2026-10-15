# Agent Session Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect Claude Code and Codex processes in each terminal PTY and render their icons in both the Navigator and terminal tabs.

**Architecture:** The daemon stores each shell PID, periodically snapshots the Windows process tree, and derives `terminal`, `claude`, or `codex` from descendants. State changes are broadcast through the existing IPC and Tauri event path; the Vue session store updates in place and a shared local-icon resolver feeds both UI locations.

**Tech Stack:** Rust 2021, portable-pty, sysinfo, Tokio, Tauri v2, Vue 3, TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-09-02-agent-session-icons-design.md`

## Global Constraints

- Detect descendant processes, not session names or terminal output.
- Retain the existing session naming scheme and PTY behavior.
- Use `parking_lot::Mutex` for backend shared state.
- Send update events only when agent state changes.
- Use local SVG icon data; do not introduce runtime icon fetching.
- Process-inspection failures render the generic terminal icon.

---

## File Structure

- `src-tauri/Cargo.toml`: add `sysinfo`.
- `src-tauri/src/pty/agent.rs`: pure descendant classifier and Windows process snapshot adapter.
- `src-tauri/src/pty/mod.rs`: `AgentKind`, `SessionInfo.agent`, and private shell PID capture.
- `src-tauri/src/pty/manager.rs`: update session agent state and return transitions.
- `src-tauri/src/ipc/protocol.rs`, `src-tauri/src/ipc/server.rs`, `src-tauri/src/lib.rs`: daemon and Tauri event flow.
- `src/lib/tauri.ts`, `src/composables/useSessions.ts`: frontend session-event bridge.
- `src/lib/session-agent-icon.ts`, `src/lib/offline-icons.ts`: local icon mapping.
- `src/lib/navigator.ts`, `src/components/SideBar.vue`, `src/components/PaneTabs.vue`: shared icon consumers.

### Task 1: Classify descendants of a PTY shell

**Files:** Create `src-tauri/src/pty/agent.rs`; modify `src-tauri/Cargo.toml`, `src-tauri/src/pty/mod.rs`; test in `agent.rs`.

**Interfaces:**

```rust
pub enum AgentKind { Terminal, Claude, Codex }
pub struct ProcessEntry { pub pid: u32, pub parent_pid: Option<u32>, pub image_name: String, pub command_line: String, pub started_at: u64 }
pub fn agent_for_shell(shell_pid: u32, processes: &[ProcessEntry]) -> AgentKind
pub fn process_snapshot() -> Vec<ProcessEntry>
```

- [ ] **Step 1: Write failing tests for nested Codex detection, newest-match selection, and no-match fallback.**

```rust
assert_eq!(agent_for_shell(10, &[entry(10, None, "pwsh.exe", "pwsh", 1), entry(11, Some(10), "claude.exe", "claude", 2)]), AgentKind::Claude);
assert_eq!(agent_for_shell(10, &[entry(10, None, "pwsh.exe", "pwsh", 1)]), AgentKind::Terminal);
```

- [ ] **Step 2: Run `cargo test --manifest-path src-tauri/Cargo.toml pty::agent::tests -- --nocapture`; confirm missing-module failure.**
- [ ] **Step 3: Add `sysinfo = "0.33"`; implement breadth-first descendant collection and case-insensitive executable/command-line matching for `claude(.exe)` and `codex(.exe)`. Select the greatest `started_at`; return `Terminal` if unmatched.**
- [ ] **Step 4: Capture `child.process_id()` before moving the child into the reader thread. Store it as private `Session.shell_pid`; initialize public `SessionInfo.agent` as `AgentKind::Terminal`.**
- [ ] **Step 5: Re-run the focused test and `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`; both must pass.**
- [ ] **Step 6: Commit with `git add src-tauri/Cargo.toml src-tauri/src/pty/agent.rs src-tauri/src/pty/mod.rs && git commit -m "feat: classify PTY agent processes"`.**

### Task 2: Publish only actual agent-state transitions

**Files:** Modify `src-tauri/src/pty/manager.rs`, `src-tauri/src/ipc/protocol.rs`, `src-tauri/src/ipc/server.rs`; test manager and protocol modules.

**Interfaces:**

```rust
pub fn refresh_agents(&self, processes: &[ProcessEntry]) -> Vec<(Uuid, AgentKind)>
Event::SessionAgentChanged { id: Uuid, agent: AgentKind }
```

- [ ] **Step 1: Write failing tests that a terminal-to-Claude change produces one transition, a second identical scan produces none, and the event serializes as `session_agent_changed` with `agent: "codex"`.**
- [ ] **Step 2: Run `cargo test --manifest-path src-tauri/Cargo.toml refresh_agents -- --nocapture`; confirm missing API failure.**
- [ ] **Step 3: Implement `refresh_agents` under the existing manager mutex. For every session, derive from `shell_pid`, mutate `info.agent` only when different, and return `(Uuid, AgentKind)` after release.**
- [ ] **Step 4: Add the event protocol variant. In `run_server`, spawn one 1-second Tokio interval before accepting clients; use `spawn_blocking(process_snapshot)`, call `refresh_agents`, and broadcast each returned change.**
- [ ] **Step 5: Run `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture`; all Rust tests must pass.**
- [ ] **Step 6: Commit with `git add src-tauri/src/pty/manager.rs src-tauri/src/ipc/protocol.rs src-tauri/src/ipc/server.rs && git commit -m "feat: publish agent state changes"`.**

### Task 3: Reconcile daemon agent events in the webview

**Files:** Modify `src-tauri/src/lib.rs`, `src/lib/tauri.ts`, `src/composables/useSessions.ts`; create `src/lib/session-agent.ts` and `src/lib/session-agent.test.ts`.

**Interfaces:**

```ts
export type AgentKind = "terminal" | "claude" | "codex";
export function applySessionAgent<T extends { id: string; agent: AgentKind }>(sessions: readonly T[], update: { id: string; agent: AgentKind }): T[];
export function onSessionAgentChanged(handler: (payload: { id: string; agent: AgentKind }) => void): Promise<UnlistenFn>;
```

- [ ] **Step 1: Write a failing `applySessionAgent` test asserting only the matching ID changes from `terminal` to `codex`.**
- [ ] **Step 2: Run `pnpm exec vitest run src/lib/session-agent.test.ts --pool=forks --maxWorkers=1`; confirm missing-module failure.**
- [ ] **Step 3: Implement the pure helper. Extend TypeScript `SessionInfo` with `agent`; forward the Rust event in `lib.rs` as `session-agent-changed`; add the Tauri listener wrapper.**
- [ ] **Step 4: Register the listener once in `useSessions.ts` next to the activity listener and update only the matching reactive session's `agent` field. Do not add a frontend polling loop.**
- [ ] **Step 5: Run the focused test, then `pnpm exec vitest run --pool=forks --maxWorkers=1`; all must pass.**
- [ ] **Step 6: Commit with `git add src-tauri/src/lib.rs src/lib/tauri.ts src/lib/session-agent.ts src/lib/session-agent.test.ts src/composables/useSessions.ts && git commit -m "feat: sync agent state to webview"`.**

### Task 4: Render shared local Claude, Codex, and terminal icons

**Files:** Modify `src/lib/offline-icons.ts`, `src/lib/navigator.ts`, `src/components/SideBar.vue`, `src/components/PaneTabs.vue`; create `src/lib/session-agent-icon.ts` and its test.

**Interfaces:**

```ts
export function sessionAgentIcon(agent: AgentKind): IconifyIcon;
```

- [ ] **Step 1: Write a failing table-driven test mapping `claude`, `codex`, and `terminal` to `claudeIcon`, `codexIcon`, and `terminalIcon`.**
- [ ] **Step 2: Run `pnpm exec vitest run src/lib/session-agent-icon.test.ts --pool=forks --maxWorkers=1`; confirm missing-module failure.**
- [ ] **Step 3: Add local SVG definitions for Claude and Codex to `offline-icons.ts`, then implement the three-branch resolver.**
- [ ] **Step 4: Include `agent` in `NavigatorSessionNode` and its input pick; update Navigator fixtures. Replace the SideBar session-row glyph with `sessionAgentIcon(s.agent)`.**
- [ ] **Step 5: Change PaneTabs so terminal tabs resolve the active session's agent icon through the same resolver. Preserve current file and browser icons, labels, drag behavior, and close behavior.**
- [ ] **Step 6: Run the focused icon/Navigator tests and `pnpm exec vue-tsc --noEmit`; all must pass.**
- [ ] **Step 7: Commit with `git add src/lib/offline-icons.ts src/lib/session-agent-icon.ts src/lib/session-agent-icon.test.ts src/lib/navigator.ts src/lib/navigator.test.ts src/components/SideBar.vue src/components/PaneTabs.vue && git commit -m "feat: show Claude and Codex icons"`.**

### Task 5: Verify complete behavior

**Files:** Modify only files required to resolve a verification failure.

- [ ] **Step 1: Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo test --manifest-path src-tauri/Cargo.toml -- --nocapture`; expect no failures.**
- [ ] **Step 2: Run `pnpm exec vitest run --pool=forks --maxWorkers=1`, `pnpm exec vue-tsc --noEmit`, and `pnpm build`; expect all to complete successfully.**
- [ ] **Step 3: In `pnpm tauri dev`, run `claude`, exit it, run `codex`, and verify both the Navigator and terminal tab change to the matching local icon within two seconds and return to terminal on exit. Verify file/browser tabs are unchanged.**
- [ ] **Step 4: Commit any verification fixes with `git add -A && git commit -m "test: verify agent session icon integration"`.**

## Self-review

- Process classification, newest-process precedence, and safe fallback are Task 1.
- Daemon-owned monitoring, unchanged-state suppression, and IPC are Task 2.
- Tauri bridge and reactive frontend reconciliation are Task 3.
- Both required UI locations use one local icon resolver in Task 4.
- Rust, frontend, production-build, and runtime checks are Task 5.
