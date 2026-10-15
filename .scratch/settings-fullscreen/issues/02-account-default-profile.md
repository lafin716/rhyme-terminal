# Spec: M2 — Default profile per agent (Accounts)

Status: ready-for-agent
Feature: settings-fullscreen
Depends on: 01 — Accounts base layout (same PR is fine; this just assumes the two-card grid from
issue 01 already exists)
See also: `../PRD.md`, `CONTEXT.md`

Design reference: https://claude.ai/code/artifacts/8d0afdeb-f737-4881-9074-d340dd764bda —
Accounts artboard.

## Problem Statement

A user can add multiple isolated login profiles per CLI agent (Claude Code, Codex — see
`CLI_AGENTS` in `useAccountProfiles.ts`), but nothing designates which one — or the agent's
ambient system login — should be used by default. `Prefs.defaultClaudeProfileId` already exists in
`usePrefs.ts`/`persistence.ts` as groundwork for a future "Launch Claude" quick action, but:

- it's `null`-or-`string`, Claude-only, and has no Codex equivalent;
- it is currently **written to storage and never read anywhere** — no UI shows or sets it, and no
  session-launch path consults it.

This issue makes it visible and settable in Accounts. It does **not** build the quick action that
would consume it (see Out of Scope) — that only becomes valuable once one exists.

## Solution

- **Generalize the Prefs field.** Both `usePrefs.ts` and `persistence.ts`'s `Prefs` interface
  currently define `defaultClaudeProfileId: string | null`. Since this field is still uncommitted
  in the working tree (no shipped users to migrate), rename/reshape it rather than add a migration:

  ```ts
  // persistence.ts
  defaultProfileId: Record<CliAgentKind, string | null>;
  ```

  ```ts
  // usePrefs.ts — initial state
  defaultProfileId: { claude: null, codex: null },
  ```

  `null` means **System**: no `CLAUDE_CONFIG_DIR`/`CODEX_HOME` override, i.e. whatever that CLI's
  own default login is — for example a prior plain `claude login` run in an ordinary terminal, with
  no winmux profile involved at all. This is the default for both agents until a profile is chosen.

- **Accounts UI**, per agent card (built on issue 01's two-card grid): add a leading "Default"
  radio column to the table. Header row becomes `Default | Profile | Login folder |`. Row order:
  1. A synthetic, **non-removable** "System" row, always first:
     - Radio: `<input type="radio" :name="`${agent.id}-default`" :checked="prefs.defaultProfileId[agent.id] === null" @change="setPref('defaultProfileId', { ...prefs.defaultProfileId, [agent.id]: null })" />`
     - Profile cell: plain text "System" + a small muted "Built-in" tag (not an editable input —
       this row can't be renamed).
     - Login folder cell: the agent's real unset-default config path —
       `C:\Users\<user>\.claude` for Claude Code (`CLAUDE_CONFIG_DIR` unset default),
       `C:\Users\<user>\.codex` for Codex (`CODEX_HOME` unset default). Don't hardcode a literal
       Windows username; resolve it the same way the rest of the app would (or read it from
       wherever the app already knows the home directory) rather than hardcoding a guess.
     - Actions: "New session" only (launches with no env override — same behavior as today's plain
       "New Terminal", just pre-filled to run that agent's CLI command via `launchCommandForProfile`-
       equivalent for the bare agent, not a profile). No "Remove".
  2. Then each `profilesForAgent(agent.id)` row as today, with a radio prepended:
     `<input type="radio" :name="`${agent.id}-default`" :checked="prefs.defaultProfileId[agent.id] === p.id" @change="setPref('defaultProfileId', { ...prefs.defaultProfileId, [agent.id]: p.id })" />`

- **Extract the resolution logic as a pure function** (new file, mirrors the existing
  `terminal-launch.ts` pattern of pure fns + co-located vitest tests):

  ```ts
  // src/lib/default-profile.ts
  import type { AccountProfile, CliAgentKind } from "../composables/useAccountProfiles";

  /**
   * The profile to use by default for `agent`, or `null` for "System" (no env
   * override) — both when explicitly set that way and when the stored id no
   * longer matches any profile (e.g. it was removed since being chosen).
   */
  export function resolveDefaultProfile(
    agent: CliAgentKind,
    profiles: AccountProfile[],
    defaultProfileId: Record<CliAgentKind, string | null>,
  ): AccountProfile | null {
    const id = defaultProfileId[agent];
    if (!id) return null;
    return profiles.find((p) => p.id === id && p.agent === agent) ?? null;
  }
  ```

  `SettingsModal.vue` doesn't strictly need this function for the radio UI itself (a direct
  `===` check against `prefs.defaultProfileId[agent.id]` is enough there), but it's the seam a
  future "Launch <Agent>" quick action would call, and it's what makes the "removed profile falls
  back to System" behavior testable in isolation.

- **Icons.** Replace the current letter badges (`C` / `X`) with two original inline SVGs — not
  either company's actual logo/mark (see Further Notes). 20×20 viewBox, `fill="none"
  stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"`:

  - Claude Code (chat bubble with a code-bracket mark) — keep the existing teal accent
    (`background: rgba(78,201,176,.12); color:#4ec9b0`):
    `<path d="M3 5.5C3 4.4 3.9 3.5 5 3.5h10c1.1 0 2 .9 2 2v6c0 1.1-.9 2-2 2H9l-3.5 3v-3H5c-1.1 0-2-.9-2-2z"/><path d="M8.2 7.3L6.4 9.5l1.8 2.2M11.8 7.3l1.8 2.2-1.8 2.2"/>`
  - Codex (a pair of curly braces) — new amber accent, since this is the first non-teal accent in
    the file: `background: rgba(209,154,102,.14); color:#d19a66`:
    `<path d="M7.2 3.8c-1.3 0-1.9.8-1.9 1.9v2.1c0 .9-.4 1.4-1.3 1.7v.5c.9.3 1.3.8 1.3 1.7v2.1c0 1.1.6 1.9 1.9 1.9M12.8 3.8c1.3 0 1.9.8 1.9 1.9v2.1c0 .9.4 1.4 1.3 1.7v.5c-.9.3-1.3.8-1.3 1.7v2.1c0 1.1-.6 1.9-1.9 1.9"/>`

## Testing Decisions

- New `src/lib/default-profile.test.ts` (vitest, same style as `terminal-launch.test.ts`):
  - returns `null` when `defaultProfileId[agent]` is `null` (System);
  - resolves the matching `AccountProfile` when the id is set and present;
  - returns `null` (falls back to System) when the stored id doesn't match any profile — e.g. the
    profile was removed since being chosen;
  - returns `null` when the stored id belongs to a profile for the *other* agent (defends against a
    future bug mixing up the two agents' ids).
- Manual verification: add a Claude Code profile, set it as default, close and reopen Settings —
  still selected. Remove that profile — default silently reverts to System, no error. Repeat for
  Codex.

## Out of Scope

- **Building the "Launch `<Agent>`" quick action** that would actually read
  `defaultProfileId`/`resolveDefaultProfile` to spawn a session — that's the reason the field
  exists, but there's no UI trigger for it yet anywhere in the app (not a button, not a keybinding).
  This issue only makes the value **visible and settable**; wiring a consumer is a separate,
  follow-up piece of work once such an action is designed.
- Auth method (OAuth vs. `setup-token`) UI, `resolveProfileEnv`, and the floating OAuth login
  window — owned by concurrent work in `useAccountProfiles.ts` / `floating-login.ts`; not touched
  here. Re-check those files' current shape before implementing, since they were mid-change when
  this spec was written.
- Changing `envForProfile` / `launchCommandForProfile` — reused as-is for the System row's "New
  session" (called with no env override, the same as a plain new terminal, just pre-seeded with
  that agent's launch command).

## Further Notes

- The badge icons are deliberately original glyphs (a chat bubble with code brackets; a curly-brace
  pair), not a reproduction of Anthropic's or OpenAI's actual marks.
- `defaultProfileId` replaces `defaultClaudeProfileId` everywhere it's referenced
  (`usePrefs.ts`, `persistence.ts`'s `Prefs`/`StoredPrefs` types and their doc comments).

## Comments

Implemented as specified, with one correction to a factual claim in the Problem Statement: it says
`defaultClaudeProfileId` "is currently written to storage and never read anywhere — no UI shows or
sets it, and no session-launch path consults it." That's no longer true as of concurrent work
already in the tree — `App.vue`'s `launchDefaultClaude()` (backing the existing `session.newClaude`
/ "Launch Claude" action, `Ctrl+B C`) already reads it: `profiles.find(...)` + `resolveProfileEnv`.

This doesn't block or change the spec's design (the rename/reshape to
`defaultProfileId: Record<CliAgentKind, string | null>` is exactly as written), it just means the
rename has a real second call site beyond `SettingsModal.vue`. I updated `App.vue` through the
rename and, since `resolveDefaultProfile` already encodes the identical
"look up by id, fall back to null if missing" logic `launchDefaultClaude` was doing inline, switched
it to call `resolveDefaultProfile("claude", profiles, prefs.defaultProfileId)` instead of
duplicating that lookup. No behavior change for that quick action beyond going through the new
field shape — did not touch its Codex-equivalent (there isn't one; out of scope per this issue).

Also implemented: `resolveDefaultProfile` (verbatim per spec) + `default-profile.test.ts` (all 4
cases from Testing Decisions), the Accounts radio column + non-removable "System" row per agent
(config path resolved via `@tauri-apps/api/path`'s `homeDir()` — already covered by this app's
existing `core:default` capability grant, no capabilities-file change needed), and the new
Claude/Codex badge SVGs replacing the letter badges. `vue-tsc --noEmit` and `vitest run` (128
tests, including the 4 new ones) both pass.

Manual verification (add profile → set default → reopen Settings → still selected; remove that
profile → falls back to System with no error) still needs a human at the actual window — I don't
have a way to screenshot/drive the native Tauri GUI from this environment. `pnpm tauri dev` does
build and launch cleanly though (confirmed, then cleaned up my verification instance since another
winmux instance was already running).
