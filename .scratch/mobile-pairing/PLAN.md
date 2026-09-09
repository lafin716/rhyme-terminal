# Mobile pairing implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development to implement task-by-task.

**Goal:** Select a local network IP and pair a mobile browser over Tailscale to existing terminals.
**Architecture:** Desktop-owned HTTP/WebSocket gateway, dedicated daemon connection per mobile socket, bundled mobile Vue/xterm entry.
**Tech Stack:** Rust/Tokio/Tauri, Vue/TypeScript/Vite/xterm.
**Spec:** `.scratch/mobile-pairing/PRD.md` (approved by user request to implement with subagents).

## Global constraints

- Preserve existing dirty work; no automatic commits or staging of mixed files.
- Bind only selected IP, port default 43123; QR derived from bound address.
- Service starts disabled, survives hiding to tray, ends on app exit.
- Invite lifetime 120 seconds, single use; explicit desktop approval; device revocation closes connections.
- Authentication before session access, strict Host/Origin, size/time/concurrency limits.
- Dedicated daemon client per socket, bounded output, no mobile PTY resize or automatic input replay.
- Korean user UI, localStorage version 1 envelope for non-secret preferences only.

## Task 1: Backend gateway

Owned files: `src-tauri/src/mobile_pairing/`, `src-tauri/src/ipc/`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/src/lib.rs`.

- [x] Add state-machine regression tests for single-use invitations, approval, expiry, revocation, exact bind origin and URL formatting.
- [x] Implement adapter enumeration, server state, QR SVG, pairing WebSocket, authenticated session protocol, limits and cleanup.
- [x] Embed assets from `dist-mobile/` using a build-compatible approach; allow building before assets exist with explicit unavailable-page response, never silently claim usable mobile build. Coordinate asset output with Task 2.
- [x] Expose Tauri commands listed in `CONTRACT.md`; register managed state and commands.
- [x] Resolve attach snapshot/live-output race and daemon reader lifetime with focused tests.
- [x] Run targeted Rust tests and compile checks; record exact results in work/backend-report.md.

## Task 2: Mobile browser

Owned files: `src/mobile/`, `mobile.html`, `vite.mobile.config.ts`, `package.json`, `pnpm-lock.yaml`, `.gitignore` if needed.

- [x] Implement protocol client with sessionStorage token, fragment removal, approval waiting, bounded reconnect and no input replay.
- [x] Implement mobile session selector, terminal output, Korean composition-safe input and auxiliary keys.
- [x] Build self-contained relative assets into `dist-mobile/`; desktop build script also builds mobile assets and dev preparation does likewise.
- [x] Test protocol reconnect/authorization/input behavior in Vitest and run frontend build; record work/mobile-report.md.

## Task 3: Desktop settings

Owned files: `src/components/MobilePairingSettings.vue`, `src/composables/useMobilePairing.ts`, `src/lib/mobile-pairing.ts`, `src/components/SettingsModal.vue`, `src/lib/locales/ko.ts`.

- [x] Define typed invokes matching contract and validated preference persistence.
- [x] Add Mobile connection settings category, adapter/IP selection, port, start/stop, URL/QR, expiry and pending approval/device revocation.
- [x] Poll status while mounted without overlapping requests; clean timers on unmount; show errors; prevent changing address while running.
- [x] Add tests for invalid persisted data and changed interface selection; run build and relevant tests; record work/desktop-report.md.

## Task 4: Review and integration

- [x] Independently review each task's scope and final combined security/concurrency behavior.
- [x] Run Rust tests, full Vitest and build; exercise HTTP/WebSocket integration and rendered mobile/settings UI when tools allow.
- [x] Add follow-up ADR and usage instructions; mark PRD implemented only with verified evidence and list hardware-only checks separately.

## Execution rulings

The working tree contains extensive unrelated edits. Work in the current checkout, preserve a baseline diff, and review named files/new files without committing mixed user work. Sequential implementation agents avoid shared-file conflicts; controller performs validation and documentation alongside each agent. No additional approval is needed for this authorized implementation.

