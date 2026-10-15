# Agent Loop: session lost when the daemon restarts mid-run

Status: ready-for-human

## Problem (as reported)

> 에이전트 루프 기능에 각 프로필들이 세션을 공유하지 못해서 만약 A 계정에서 작업 진행 중
> 사용량도 초과하고 데몬 자체가 꺼졌다가 켜지면 다시 루프를 실행할때 다음 계정으로
> 켜지게 되는데 resume 을 해도 세션을 찾을 수가 없음.

While Loop is running Account A and A's usage limit trips *at the same time*
the `winmuxd` daemon itself is killed and restarted (crash, forced update,
`taskkill`, machine sleep/resume), reopening the app and clicking **Resume**
switches to the next account as expected, but the previous conversation can
no longer be found/resumed — the user loses the working session entirely.

Switching to the next eligible account when A is over its usage threshold is
*correct, existing behavior* (`Engine::choose`/`eligible` in
`src-tauri/src/loop_routing/runtime.rs`). The bug is that the switch, when it
happens right after a daemon restart, does not carry the previous session
forward — it behaves as a brand-new start instead of a continuation.

## Root cause

The Loop already has a working cross-account continuation design:
- `Group.current_agent_session_id` / `Group.attempts[].reference` record which
  conversation is "in flight", independent of which account ran it.
- `activate()` (`runtime.rs`) finds that conversation on disk under **any**
  enabled candidate's own profile directory
  (`adapter::conversations`/`profile_dir`), then either imports the transcript
  into the newly-selected account's config dir for a native `--resume`
  (`adapter::import_transcript`) or builds a portable text handoff
  (`adapter::build_handoff`) when native resume isn't possible.
- The **only** thing that decides whether an Agent launch is treated as "carry
  the previous conversation forward" vs. "start fresh" is the in-memory
  `Live.continuation` flag (`runtime.rs`, `struct Live`). It is never
  persisted — it is process-local and resets to `false` every time the
  daemon restarts (`live: HashMap::new()` in `Engine::open`).
- On daemon restart, `Engine::open_at` finds every Group that was actively
  managed (`running`/`preparing`/`switching_profile`/`handoff`/`resuming`/
  `error`) and forces it to **`Idle`**, with the message *"이전 프로세스가
  없습니다. 터미널에서 Agent를 실행하세요"* ("No previous process. Run the
  Agent in the terminal.") — literally inviting the user to retype
  `claude`/`codex` in the terminal.
- That retyped command is picked up by the shell wrapper
  (`scripts/loop-shell.ps1` → `bridge::run`) as an ordinary new dispatch. In
  `step()`'s dispatch handler, because the fresh `Live::default()` has
  `continuation == false`, this branch fires:

  ```rust
  if !live.continuation {
      g.current_agent_session_id = None;
      g.handoff_context = None;
      g.active_profile = None;
      ...
  }
  ```

  This wipes the exact fields `activate()` needs to find and resume Account
  A's conversation, *before* profile selection even runs. By the time the
  Loop picks the next eligible account (A is over threshold, so B is chosen —
  the part the user observed as "starts on the next account"), there is
  nothing left to resume from, so it starts a brand-new, empty conversation
  under B.
- Clicking **Resume** in the UI (`op: "resume"`) *does* correctly set
  `live.continuation = true` first — so if the user only ever uses that
  button and never retypes the command by hand, this path already works.
  But `Idle` is the wrong landing state for a Group with unfinished work: it
  presents no "you must Resume, don't retype" affordance, and the natural,
  differently-worse-case, muscle-memory action (typing the command directly,
  which is a fully supported normal flow for this Loop otherwise) silently
  discards the recovery data.
- Secondary compounding failure: even through the correct **Resume** path,
  `Handoff` calls `adapter::build_handoff` → `records()`, which requires
  *every* line of the account's conversation JSONL to parse as JSON. If the
  daemon (and, with it, the native CLI it was supervising) was killed
  abruptly rather than exiting on its own, the final JSONL line can be
  half-written. `records()` previously treated *any* unparseable line,
  including that truncation artifact, as "Session context를 확인할 수 없어
  자동 재실행을 중단했습니다" (fatal) instead of "drop the incomplete last
  line and use everything before it."

Net effect: "각 프로필들이 세션을 공유하지 못한다"는 진단은 정확했다 — 계정
간 세션 이전 로직 자체는 존재하지만, 데몬 재시작 직후에는 그 로직을 트리거하는
데 필요한 `continuation` 신호가 살아남지 못했고, 복구 메시지가 그 신호를 깨는
바로 그 동작(터미널에 직접 명령 입력)을 안내하고 있었다.

## Fix

1. **`runtime.rs::Engine::open_at`** — when a Group is downgraded because the
   daemon can no longer vouch for its previous process, land on **`Paused`**
   instead of `Idle` whenever there is something to lose
   (`current_agent_session_id.is_some()` or any attempt carries a
   `reference`). `Paused` already renders the **Resume** button
   (`LoopGroupView.vue`) and, critically, its manual-dispatch path
   (`paused`/`stopped` branch in `step()`) is an explicit, clearly-unmanaged
   escape hatch rather than something that looks like the normal managed
   flow. Groups with no session history yet still land on `Idle` as before —
   no behavior change for the common case.
2. **`adapter.rs::records()`** — tolerate a truncated/unparseable **final**
   line only (the abrupt-kill artifact), still hard-failing on a bad line
   anywhere earlier in the file (a real corruption, not a kill artifact).

Both changes are additive/narrowing (they only change what happens to a
Group that is already broken today) and are covered by new unit tests:
- `runtime_tests.rs::daemon_restart_recovers_an_in_flight_session_to_paused_not_idle`
- `runtime_tests.rs::daemon_restart_with_no_session_history_still_lands_on_idle`
- `adapter.rs::tests::records_tolerates_only_a_truncated_final_line`

## Follow-ups (not implemented here, filed as separate issues)

See `issues/`.
