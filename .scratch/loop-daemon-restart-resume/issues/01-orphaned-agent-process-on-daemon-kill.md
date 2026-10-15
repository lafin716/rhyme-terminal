# Orphaned Agent process survives an abrupt daemon kill

Status: needs-triage

## Problem

`spawn_session` (`src-tauri/src/pty/mod.rs`) spawns the Loop's shell (and,
through it, the native `claude`/`codex` process) via `portable-pty`'s ConPTY
backend with no Windows Job Object association. If `winmuxd` itself is
forcibly terminated (crash, forced update, `taskkill`, OOM) rather than
exiting normally, Windows does **not** automatically kill the shell or the
Agent process underneath it — they become orphans that keep running,
independent of the daemon instance that restarts afterward.

This matters for `../PRD.md` (Agent Loop session lost on daemon restart)
beyond the session-recovery fix already applied there:
- The orphaned native CLI can still be mid-write to the very conversation
  JSONL file that a restarted daemon's Resume flow later tries to read
  (`adapter::records`) or import (`adapter::import_transcript`), racing the
  read.
- It can still be consuming that account's usage quota invisibly, so a
  freshly-computed "is Account A over its threshold" check can be stale by
  the time the new attempt starts.
- Two processes (the orphan and the new attempt) could end up appending to
  the same session id concurrently if the user or the Loop re-selects the
  same account.

## Proposed direction

Wrap the spawned shell in a Windows Job Object created with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, assigned to the child immediately after
`spawn_command` in `pty::spawn_session`, with the Job handle owned by
`winmuxd` (not inherited by the child). This guarantees the OS kills the
whole descendant tree — shell, and the native CLI under it — the moment the
daemon process itself goes away, for any reason, without daemon-side cleanup
logic needing to run at all.

This needs care (and its own test, likely similar in spirit to
`process_controller::tests::terminates_only_the_captured_test_child`, but
provoking an actual `TerminateProcess` of the daemon's own process from a
child test harness rather than a graceful `Drop`) before merging, hence
filed separately rather than bundled into the resume fix.
