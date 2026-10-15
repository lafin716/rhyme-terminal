# PRD: settings-fullscreen — expand Settings into a full-screen view

Status: needs-triage
Owner: jupark

## Context

Design reference: https://claude.ai/code/artifacts/8d0afdeb-f737-4881-9074-d340dd764bda —
a Claude Design canvas with 5 static full-screen mockups (Terminal, Accounts, Workspaces,
Keybindings, Palette). Every color, font and spacing value in the mockup is lifted directly from
the current `SettingsModal.vue`; there are no new design tokens except one new accent pair for the
Codex badge (see issue 02).

Today `SettingsModal.vue` renders as a fixed 720×520 centered dialog (`.backdrop` + `.modal`) with
a 160px category rail and a single-column `.panel`. This effort expands it to fill the app window
and reflows each category's content into layouts sized for the extra width (multi-column
cards/grids instead of stacked single-column rows).

Two milestones:

- **M1** is a pure presentational refactor — same settings, same data, new layout. No new Tauri
  commands, no protocol changes, no new Prefs/persistence fields.
- **M2** adds one new capability to Accounts: a per-agent **default profile** picker, including an
  always-present "System" choice (the CLI's ambient login outside any winmux-managed profile). This
  generalizes `Prefs.defaultClaudeProfileId`, which already exists in `usePrefs.ts`/`persistence.ts`
  as groundwork for a "Launch Claude" quick action but is currently written and never read anywhere.

## Non-goals

- No new Terminal/Workspaces/Keybindings/Palette settings — layout changes only.
- Not a redesign of the account **add/login** flow. `useAccountProfiles.ts` and `floating-login.ts`
  are under active concurrent work elsewhere (OAuth floating-login window, `setup-token` auth
  method) — both issues here compose with whatever that flow ends up looking like and must not
  fight it. Re-read those files before touching Accounts markup, since they may have moved since
  this was written.
- No new Tauri/daemon commands; everything here is `src/` (Vue/TS) only.

## M1 — Full-screen shell + per-category layout

Replace `.backdrop`/`.modal` with a full-window shell; widen the category rail and give it
icons + descriptions; reflow each category panel into a layout sized for the extra width. See
`issues/01-fullscreen-shell-and-layout.md`.

## M2 — Default profile per agent (Accounts)

Add a "Default" radio column to each agent's profile table, with a non-removable "System" row
(no env override) selected by default. Generalize `Prefs.defaultClaudeProfileId` to both agents.
See `issues/02-account-default-profile.md`. Builds on M1's Accounts table shape.

---

## Verification (both milestones)

`pnpm tauri dev` → open Settings (Ctrl+, / `settings.open`). Confirm:

- Settings fills the window at typical desktop sizes (1600×900+) with no horizontal scroll in any
  of the 5 categories.
- Escape still closes it; there is no more click-outside-to-close backdrop (the dialog now covers
  the whole window, so there's nothing to click outside of).
- Accounts shows a "System" row per agent, radio-selectable, showing the agent's real default
  config path, with a working "New session" and no "Remove".
- Picking a different default profile and reopening Settings keeps the choice (persisted via
  `localStorage` under `winmux:prefs:v1`).
- Removing the profile currently set as default falls back to "System" without error.

## Open question for triage

"Profile" (a saved CLI account login) isn't yet a `CONTEXT.md` glossary term. Worth adding next
time `/domain-modeling` touches this area — not a blocker for either milestone here.
