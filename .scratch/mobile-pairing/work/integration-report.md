# Final integration verification

## Backend scope

- Added an internal connection factory to Gateway. Production defaults to a new `DaemonClient::connect_mobile()` per authenticated WebSocket.
- Made `DaemonClient::from_pipe_capacity` crate-visible so the integration fixture can use the actual reader, pending-request handling, event receiver and cleanup behavior on a random test pipe.
- The test-only factory creates a fresh `\\.\pipe\winmux-mobile-integration-{uuid}` pipe for every socket. The fake daemon accepts only list, atomic attach, input and detach; records every actual command and decoded input; tracks active/opened pipes. No existing daemon or user session is contacted, restarted or written to.

Positive WebSocket test covers:

1. Invite, matching pending verification code, desktop approval and authenticated push.
2. List returns the fixture session.
3. Atomic attach result contains snapshot and original 120x30 geometry; subsequent live output arrives as a separate event after the result.
4. Korean text plus CR, Tab, Escape and Ctrl+C arrive byte-for-byte; input acknowledgment is null.
5. WebSocket disconnect drops its dedicated pipe.
6. Device token re-authenticates on a different/new pipe; input before reattach is refused and never replayed.
7. Reattach restores snapshot/live output; desktop revocation closes the pipe and rejects token reuse without creating a new daemon client.
8. Input sent after revoke is never forwarded. The contract permits a best-effort error followed by closure; sending during closure can instead produce TCP reset on Windows, which the test accepts as terminal transport closure, while still requiring no extra daemon input/methods and zero active pipes.
9. Another approved socket is closed by server stop and all device credentials are cleared.
10. Exact observed daemon methods are list, attach, input, attach: no resize/create/kill/file method is used.

Embedded assets test uses current dist-mobile and verifies root HTTP 200, HTML content type and asset references, and byte-identical JavaScript/CSS assets with their correct MIME types. Direct Rust builds without mobile assets retain the separately tested explicit HTTP 503 fallback.

## Frontend review fix

Files: `src/mobile/client.ts`, `src/mobile/client.test.ts`.

Cause: pty_exit checked only state.attached.id, which is null while asynchronous onAttach applies a new/reconnected snapshot. Such an exit was discarded; automatic restore could later revive the exited session.

Fix: recognize the current applying snapshot just as output handling does; clear desired session, invalidate snapshotSequence, discard buffered output, stop applying state and clear attached. Automatic restore rechecks desiredSessionId after awaiting applyAttach.

Two deferred onAttach regressions cover initial attach and reconnect. They send output and pty_exit while the snapshot promise is pending, verify the snapshot guard becomes false, release the promise, and require attached=null, no buffered output delivered and sendInput rejected without a new wire request.

## Results

- `pnpm.cmd exec vitest run src/mobile/client.test.ts`: 7 passed (5 existing + 2 regressions), 2.39s. Initial attach regression failed on the original source (current() remained true), then passed with the fix.
- `pnpm.cmd exec vue-tsc --noEmit`: passed.
- `git diff --check -- src-tauri/src/mobile_pairing/gateway.rs src-tauri/src/ipc/client.rs src/mobile/client.ts src/mobile/client.test.ts`: passed; LF/CRLF notice only.
- First targeted Rust integration run: embedded-assets test passed; positive WS test reached revocation but expected an error frame where Windows returned TCP reset after a post-revoke write. Assertion updated to the contract's terminal closure alternatives while retaining command/input/pipe/token checks.
- Final all-targets Rust session 2268 is running; final result appended below when complete.

Controller should rebuild mobile assets after this frontend fix before final app/bundle verification. Hardware Tailscale QR, physical phone keyboard and real PTY rendering remain separate checks.

## Final Rust result

`cargo test --all-targets --manifest-path src-tauri/Cargo.toml --target-dir target-mobile -j 3` completed with exit 0 (session 2268): library 71 passed/0 failed (29.52s), CLI 23 passed/0 failed (0.03s), GUI/daemon compile and 0-test targets passed. Total 94 passing Rust tests. Both new positive integration/embedded asset tests passed; no missing-mobile-assets warning occurred in this build.

The controller's later frontend rebuild changes embedded bytes, so rerun the final Rust verification after that rebuild to include the final frontend snapshot-exit fix in the bundled assets.
