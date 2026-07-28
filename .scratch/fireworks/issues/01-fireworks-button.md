# Spec: Fireworks button (top-right celebratory effect)

Status: ready-for-agent
Feature: fireworks
Depends on: none
See also: `CONTEXT.md`, `docs/adr/0001-terminal-multiplexer-not-ade.md`

## Problem Statement

As a winmux user I spend all day in a very utilitarian terminal Shell. There is no lightweight, playful moment anywhere in the app — nothing to fire off when a build passes, a demo lands, or I just want a small celebration. The top bar has room in its right corner (next to the Explorer toggle) that could host a one-tap "party" affordance.

## Solution

Add a permanent **fireworks** button to the right corner of the top-bar (`MenuBar`). Clicking it plays a short (~2.5s) fireworks show over the whole window: several shells rise from the bottom, burst radially, and rain sparks that fall under gravity and fade out. The effect is a pure visual decoration — it never blocks the terminal, plays no sound, and has no persisted setting. Clicking again while it plays launches more shells (additive). This stays within ADR-0001: it is chrome decoration on the terminal multiplexer, not agent/task tooling.

## User Stories

1. As a winmux user, I want a clearly visible button in the top-right corner, so that I can trigger a celebration with one click.
2. As a winmux user, I want a click to play a short fireworks show (a few seconds of several bursts), so that it reads as a celebration rather than a single blip.
3. As a winmux user, I want to keep typing and clicking in my terminal while fireworks play, so that the effect never interrupts my work.
4. As a winmux user, I want clicking again mid-show to add more fireworks, so that rapid taps feel like a bigger celebration.
5. As a winmux user, I want the effect to clean up after itself, so that it leaves no residue on screen and consumes no CPU once it finishes.
6. As a winmux user with reduced-motion preferences set, I want a gentler/shorter effect, so that the animation respects my accessibility settings.

## Implementation Decisions

- **Term.** The feature and its vocabulary are `fireworks` throughout (component, composable, icon, glossary). Do not call it "confetti" — that is a different visual and is explicitly avoided in `CONTEXT.md`.
- **Trigger button in `MenuBar`.** Add one `.corner-toggle` button in the right group of `src/components/MenuBar.vue`, placed after `.spacer` and immediately to the **left** of the existing Explorer (right-panel) toggle so the panel toggle keeps the extreme corner. It reuses the existing `.corner-toggle` markup/CSS and an `<Icon>`. Unlike the panel toggles it has no persistent `.active` state — it is a momentary trigger (a brief click feedback only). Wire `@click` directly to the composable's `launch()`, following the `toggleRight`/`toggleLeft` binding pattern.
- **New icon.** Add a `fireworksIcon: IconifyIcon` to `src/lib/offline-icons.ts` (inline SVG `body` with `fill="currentColor"`, matching `panelRightIcon`/`bellIcon`) so it inherits the button's `color`.
- **Overlay component.** New `src/components/FireworksOverlay.vue` renders a `<canvas>` at `position: fixed; inset: 0; z-index: 1200` (above the current top layer, ConfirmModal at 1100) with `pointer-events: none` so terminal input/clicks pass through (the `PalettePopover` `.radial-root` precedent). It owns the `<canvas>` element and a `requestAnimationFrame` loop; the loop starts on the first live particle and **stops itself when no particles remain** (idle CPU 0), matching the fact that the codebase has no standing rAF loop today.
- **Mount point.** Add `<FireworksOverlay />` as a sibling of `QuickOpen` / `ConfirmModal` at the bottom of the `.app` container in `src/App.vue`, following the existing overlay-placement convention.
- **Pure particle model as the test seam.** Put the particle math in a pure module `src/lib/fireworks.ts` (spawn a burst → particle list; step(particles, dt) → apply gravity/velocity, decrement life, drop dead ones). `FireworksOverlay.vue` only does canvas draw + rAF timing and calls these pure functions. This mirrors the repo convention of pure logic in `src/lib/*` wrapped by thin Vue layers (e.g. `shell-panels.ts` under `useShellPanels`).
- **Transient state composable.** New `src/composables/useFireworks.ts` exposes module-scope transient state plus `launch()` (append a new salvo to the active show), following the `useQuickOpen` / `useConfirm` transient-state pattern. No persistence: the button is always on, there is no on/off preference, so `Prefs` and `winmux:*:v1` storage are untouched.
- **Colors, hardcoded.** Bright multi-color palette (reds, gold, blue, green, purple, the teal accent `#4ec9b0`, etc.) chosen at random per shell, hardcoded in the component/lib — the codebase uses no CSS custom properties, so follow the literal-color convention.
- **No keybinding.** The effect is button-only. Do not touch `src/lib/keybindings.ts` `ACTIONS` or `App.vue` `ACTION_HANDLERS`.
- **Accessibility.** Respect `prefers-reduced-motion: reduce` by shortening/weakening the show (fewer shells, shorter life, or a single muted burst).

## Testing Decisions

- **Primary verification is manual** (see Further Notes) — the canvas rendering, rAF timing, `pointer-events` pass-through, and z-ordering are visual behaviors covered by running the app.
- **One headless unit seam via Vitest** (the runner introduced in the orca-shell M1), targeting the pure `src/lib/fireworks.ts`:
  - `spawnBurst(...)` returns the expected number of particles with initial upward velocity and positive lifetimes.
  - `step(particles, dt)` decreases each particle's remaining life and applies gravity to vertical velocity.
  - particles whose life reaches zero are removed, so a show run forward enough steps drains to an empty list (guarantees the "cleans up / CPU returns to 0" story).
- **Out of test scope:** canvas draw calls, rAF scheduling, DOM mounting, and `prefers-reduced-motion` branching — manual only.

## Out of Scope

- Any date/season gating or promotional scheduling — the button is a permanent, always-available fun element.
- A Settings/Prefs on-off toggle or any persisted state.
- Keyboard-shortcut or Prefix-key triggering.
- Sound / audio effects.
- Configurable duration or intensity.
- Adding a particle/animation library (`canvas-confetti`, `tsparticles`, etc.) — the effect is hand-rolled with `<canvas>` + rAF.
- Confetti (falling paper) styling — the visual is fireworks (rise → radial burst → gravity fall).

## Further Notes

- Vocabulary follows `CONTEXT.md`; a `Fireworks` glossary entry is added alongside this spec. The feature is chrome decoration and does not violate ADR-0001's "terminal multiplexer, not an ADE" stance.
- Manual verification: run `pnpm tauri dev`; click the top-right fireworks button and confirm a ~2.5s multi-burst show; while it plays, type into and click the terminal to confirm it is non-blocking; click again mid-show and confirm more shells appear (additive); after it finishes confirm the canvas is clear and the rAF loop has stopped. Run `pnpm build` to confirm types/build pass, and `pnpm test` for the `src/lib/fireworks.ts` unit cases.
