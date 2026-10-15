# Spec: M1 — Full-screen shell + per-category layout

Status: ready-for-human
Feature: settings-fullscreen
See also: `../PRD.md`, `CONTEXT.md`

Design reference: https://claude.ai/code/artifacts/8d0afdeb-f737-4881-9074-d340dd764bda (all 5
artboards). Open it and look before implementing — this spec calls out the concrete diffs but the
canvas is the source of truth for spacing/proportion.

## Problem Statement

`SettingsModal.vue` is a fixed 720×520 centered dialog. Every category's content was designed for
that cramped column (single-column stacked cards, an 82px label gutter, a 160px category rail) and
now wastes most of a modern window. Expand it to fill the app window and reflow each category into
a layout that uses the space, without changing any setting's behavior.

## Solution

### Shared shell

- Replace `.backdrop` (dimmed overlay + centered flex) with a full-viewport container: `position:
  fixed; inset: 0; z-index: 1000; background: #1e1e1e;` — no dimming layer, since the dialog now
  covers the whole window. Drop `onBackdrop`/the mousedown-to-close handler entirely: there is no
  more click-outside surface. Keep `onEscape` (Escape still closes) unchanged.
- `.header` → `.topbar` (72px tall, `padding: 0 32px`, `border-bottom: 1px solid #111`):
  - Left: a 36×36 rounded-8 tile (`background:#202020; border:1px solid #2a2a2a; color:#4ec9b0`)
    holding a small inline SVG (split-pane glyph, matches the multiplexer concept — not a real
    app-icon asset):
    `<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="2.5" width="15" height="15" rx="2"/><path d="M10 2.5v15M2.5 11h7.5"/></svg>`
    followed by a title block: "Settings" (17px/600/#e6e6e6) + a new small subtitle "winmux
    configuration" (12px/#888).
  - Right: an `Esc` hint chip (mono, 11px, bordered — documents the existing Escape-to-close
    behavior) + the existing `.close` button, just enlarged to a 32px hit target with a hover
    circle (`background:#202020`).
- `.cats` → `.nav`, widened **160px → 280px**. Each `.cat` row becomes icon + label + a new
  one-line description, e.g.:
  ```html
  <div class="nav-item active">
    <div class="nav-icon">...svg...</div>
    <div class="nav-text">
      <div class="nav-label">Terminal</div>
      <div class="nav-desc">Shell &amp; workspace overrides</div>
    </div>
  </div>
  ```
  Active state changes from flat `background:#2a2a2a; color:#4ec9b0` to a 3px left accent border
  (`border-left-color:#4ec9b0`) plus the same background/icon/label tint — same active-state
  semantics (`:class="['nav-item', { active: activeCategory === 'terminal' }]"`), styling only.
  Descriptions, one per category (static text, not props):
  - Terminal — "Shell & workspace overrides"
  - Accounts — "Multi-account CLI logins"
  - Workspaces — "Per-workspace default folder"
  - Keybindings — "Shortcuts & prefix keys"
  - Palette — "Quick command menu"
- `.panel` → `.content` / `.content-inner`: padding `12px 16px` → `40px 48px 56px`; `overflow: auto`
  unchanged. Every category's top now starts with a `.panel-header` (title 20px/600 + the existing
  `.hint` text, now capped `max-width: 640px` so hint copy doesn't stretch edge-to-edge) and,
  where a category had a header-level action button (Keybindings' "Reset all", Palette's "+ Add
  item"), a `.panel-actions` slot on the right of that same header row instead of the old
  `.kb-header` flex row.

Icon set for the 5 nav items (20×20 viewBox, `fill="none" stroke="currentColor" stroke-width="1.6"
stroke-linecap="round" stroke-linejoin="round"`, all originals — not any third-party icon set):

- Terminal: `<rect x="2.5" y="3.5" width="15" height="13" rx="2"/><path d="M6.5 8l3 2.2-3 2.2"/><path d="M11 12.4h3"/>`
- Accounts: `<circle cx="10" cy="7.2" r="3"/><path d="M4.2 16.5c0-3.2 2.6-5.2 5.8-5.2s5.8 2 5.8 5.2"/>`
- Workspaces: `<path d="M2.5 5.8c0-.7.6-1.3 1.3-1.3H8l1.6 2h6.6c.7 0 1.3.6 1.3 1.3v6.9c0 .7-.6 1.3-1.3 1.3H3.8c-.7 0-1.3-.6-1.3-1.3z"/>`
- Keybindings: `<rect x="2.2" y="5.5" width="15.6" height="10" rx="1.8"/><path d="M5.8 12h8.4"/>` plus
  four small filled dots (`<circle r="0.35" fill="currentColor" stroke="none">`) at
  `(5.6,8.6) (8.4,8.6) (11.2,8.6) (14,8.6)`
- Palette: two overlapping rounded squares — `<rect x="3.5" y="3.5" width="9.5" height="9.5" rx="2"/><rect x="7" y="7" width="9.5" height="9.5" rx="2"/>`

### Terminal category

- Global default card unchanged in content, wider field grid: label column `82px → 120px`.
- Workspace overrides: today a vertical stack of full-width `.workspace-terminal` cards. Becomes a
  3-up grid (`display:grid; grid-template-columns: repeat(3, minmax(0,1fr)); gap:20px`). Each card
  keeps its checkbox + workspace name + "Active" tag header; when the override is enabled, show a
  **compact** 2-column field grid (68px label) with Preset + Program only — drop the per-workspace
  Arguments editor from this compact card (keep it only in the Global Default card above) so 3 cards
  fit per row without the grid getting too tall. Flag this as a deliberate scope-down if anyone
  relied on editing per-workspace args from a cramped card — it was barely usable at 720px anyway.

### Accounts category

Base layout only here (the default-profile picker is issue 02, built on top of this):

- Two cards side by side (`grid-template-columns: repeat(2, minmax(0,1fr))`), one per
  `CLI_AGENTS` entry, instead of a single-column stack. Reuse `profilesForAgent(agent.id)` as today.
- Table columns stay Profile / Login folder / actions, just wider (row padding `8px 6px → 12px
  16px`); widen the path cell so less of `configDir` gets ellipsis-truncated than the current
  720px layout allows.

### Workspaces category

- Stays a single wide table (not cards — there's no benefit to a grid here). Column widths grow:
  workspace name ~240px (badge 22px→30px, rounded-5→rounded-8), path column flexible, actions auto.
  Row padding `8px 6px → 16px 20px`.

### Keybindings category

- Replace the single vertical 23-row table with 3 grouped cards by `ActionDef.category`
  (`session`, `pane`, `window`+`settings` sharing a third column), `grid-template-columns:
  repeat(3, minmax(0,1fr))`. Each card: uppercase category name + a count chip (8 / 10 / 4+1), then
  rows of label + key-cell + an icon-only reset button (same `resetBinding(a.id)` handler, just an
  icon instead of the text "Reset" button — drop the text "Clear" button from the row to fit three
  columns; keep `clearBinding` reachable some other way if it's used often, e.g. a right-click or a
  small overflow menu — flag this as a judgment call to confirm with whoever owns keybindings UX).
  "Reset all" moves from the old `.kb-header` row into the new `.panel-actions` slot — same
  `resetAll()` handler.
- The conflict warning (`v-if="conflicts.size > 0"`) is unchanged logic-wise — just restyle to the
  new content width. It won't show with today's default bindings (none conflict); it's there for
  when a user's custom rebinding collides.

### Palette category

- "Display style" `<select>` (`context` / `radial`) becomes two clickable tiles side by side, each
  with a small inline-SVG preview (a 3-line list glyph for "Context menu", a 7-dot ring glyph for
  "Radial menu") and a one-line description. Same binding, different control:
  `@click="setPref('paletteUiMode', 'context')"` /
  `@click="setPref('paletteUiMode', 'radial')"`, with `:class="{selected: prefs.paletteUiMode === 'context'}"`.
  Use `<button type="button">` for each tile, not a bare `<div>`, so it's keyboard/AT accessible.
- "+ Add item" moves into `.panel-actions` next to the title, same as Keybindings' "Reset all".
- Items table: same 4 columns (Label / Command / Auto-run / actions), wider grid + larger row
  padding.

## Implementation Decisions

- This is CSS/markup only inside `SettingsModal.vue` — no composable or persistence changes, no
  new props, no new v-model targets beyond what already exists (`activeCategory`,
  `prefs.paletteUiMode`, the keybinding handlers, etc.).
- Prefer literal hex values matching the file's existing convention (it doesn't use CSS custom
  properties today) rather than introducing a token/variable system — keeps the diff about layout,
  not a styling-architecture change nobody asked for.
- Colors, font stack (`Inter, "Segoe UI", sans-serif`), and mono stack (`Consolas, "Cascadia
  Mono", monospace`) are unchanged from today's file — no new fonts.

## Testing Decisions

- No new pure logic here (presentational refactor), so no new `.test.ts` file for this issue.
  Verify manually per the PRD's Verification section at a few widths (1600×900, 1920×1080, and a
  narrower ~1280×800 laptop size) to confirm nothing clips or overlaps.

## Out of Scope

- The Accounts default-profile picker (issue 02).
- Any change to `useAccountProfiles.ts`, `floating-login.ts`, `useKeybindings.ts`,
  `usePalette.ts`, or `useWorkspaces.ts` logic — this issue only touches `SettingsModal.vue`'s
  template/style.
- Deciding where "Clear binding" goes once it's dropped from the Keybindings row — noted above as
  a follow-up decision, not resolved here.

## Further Notes

- Vocabulary: `Workspace`, `Session` per `CONTEXT.md`. "Profile" (CLI account profile) isn't yet a
  glossary term — see the PRD's open question.

## Comments

Implemented in full against the extracted canvas artboards (all 5, pulled directly from the
published Claude Design artifact's `content.files` payload — `Main.dc.html`, `Accounts.dc.html`,
`Workspaces.dc.html`, `Keybindings.dc.html`, `Palette.dc.html`). Two spots where the spec text and
the concrete canvas markup diverge and I had to make a judgment call — flagging both for
confirmation from whoever owns this UX, per the spec's own note on the first one:

1. **Keybindings row actions.** The spec text says to drop the text "Clear" button and "keep
   `clearBinding` reachable some other way ... e.g. a right-click." I implemented exactly that:
   each key-cell now has a `@contextmenu.prevent` handler that calls `clearBinding`, with the
   cell's `title` updated to "Click to record • Right-click to clear". This was the spec's own
   suggested fallback, so I built it rather than leaving `clearBinding` orphaned (which would also
   have failed the strict `noUnusedLocals` build). Please confirm right-click-to-clear is
   discoverable enough, or say if you'd rather have a small overflow menu instead.

2. **Terminal workspace-override "Program" field.** The written spec only calls out dropping the
   Arguments editor from the compact per-workspace card ("Preset + Program only"). The concrete
   canvas markup for that compact card, though, shows the Program field as a bare input with **no
   Browse button** (no `.inline-row` wrapper) — unlike the Global Default card's Program field,
   which keeps Browse. I followed the canvas literally here (source of truth for layout per this
   issue's own framing), so `chooseTerminalProgram()` no longer takes a `workspaceId` and is only
   reachable from the Global Default card; a workspace override's Program value can still be typed
   directly, just not filesystem-browsed. Flagging in case dropping Browse there was an oversight
   in the mockup rather than intentional.

Everything else (shell/topbar/nav restructure, per-category grids, icons, panel-header/actions
slot) matches the spec and canvas as written. `vue-tsc --noEmit` and the full `vitest run` suite
pass; `pnpm tauri dev` builds and launches cleanly (Rust side untouched, so no surprises there).
Manual visual pass across 1280×800/1600×900/1920×1080 is still pending a human with eyes on the
actual window — see the PRD's Verification section.
