# Backend implementation report

## Status

Task 1 implementation complete; final all-targets test is running (session 57732). Full library test passed: 69 passed, 0 failed, 22.46s (session 3587). This includes 13 new mobile/IPC regressions.

## Owned changes

- `src-tauri/src/mobile_pairing/mod.rs`: adapter enumeration, exact chosen-IP bind, canonical IPv6/default-port Origin handling, disabled-by-default service lifetime, 120-second single-use invitation, explicit approval, bounded pending/device records, in-memory device credentials, revocation cancellation, seven Tauri commands.
- `src-tauri/src/mobile_pairing/gateway.rs`: embedded asset HTTP service and authenticated WebSocket protocol; bodyless GET routes, Host/Origin validation, protocol/event whitelist, bounded connections/timeouts/rate/queues, no resize/create/kill/file exposure.
- `src-tauri/src/ipc/client.rs`: reader abort-on-drop, pending-request RAII cleanup for write error/timeout/cancellation, disconnect signal, mobile queue capacity 64, response-boundary event receiver returned to caller before next event dispatch.
- `src-tauri/src/ipc/server.rs`: `AttachSessionAtomic`, snapshot plus broadcast subscription under scrollback lock; complete prior output frame before replacing relay (writer lock then abort/join); relay starts after response; mobile lag/slow pipe are fatal; original desktop attach remains available.
- `src-tauri/src/ipc/protocol.rs`: additive internal daemon method only. Browser contract unchanged.
- `src-tauri/src/pty/mod.rs`: controller-approved minimal ownership extension: publish output under the same scrollback lock used by the atomic snapshot.
- `src-tauri/src/lib.rs`: managed state and commands. Existing usage-related user changes preserved.
- `src-tauri/Cargo.toml`, `Cargo.lock`: axum/hyper/hyper-util/tower, if-addrs, qrcode; test-only tokio-tungstenite/futures-util. Existing reqwest dependency preserved.
- `src-tauri/build.rs`: generate OUT_DIR static include_bytes table from dist-mobile; missing mobile.html yields explicit build warning and HTTP 503 page. Assets are embedded at Rust compile time. Rebuild Rust after frontend assets change.

## Limits

- 32 HTTP connections, 5s HTTP header timeout, 16KB HTTP header buffer, 10s HTTP connection lifetime.
- 16 WebSockets globally, 30 upgrade/auth attempts per minute globally, 10s first authentication message timeout.
- Invitation and pending approval expire at invitation deadline (120s); 8 pending and 16 paired devices maximum, 4 sockets/device.
- Device names 1-64 Unicode characters, control characters rejected; 244 random bits in concatenated UUID-v4 credential.
- Client frames/messages <=24KB, UTF-8 input decoded <=16KB, <=60 commands/second/socket.
- Mobile daemon broadcast capacity 64 events (typical max PTY chunk 8192 bytes => ~0.7MB encoded event storage per mobile client); lag closes connection instead of silently dropping output.
- WS send and pipe event send deadline 5s; daemon request deadline 10s; no input replay.

## Verification completed

`cargo test --lib --manifest-path src-tauri/Cargo.toml --target-dir target-mobile -j 3`

Result: 69 passed, 0 failed (22.46s), including:

1. Invite consumed before approval; reuse and unauthenticated token rejected.
2. Expiration/rejection never issue credentials.
3. Revocation removes token and signals connected watchers.
4. Exact Host/Origin and IPv6 URL formatting.
5. HTTP selected Host, Origin, root asset availability, unknown path, query rejection.
6. WS Origin required; list-before-auth and bad token rejected without session data.
7. Pending invitation single-use; closing socket removes pending request.
8. Rejection delivered to pending socket; stopping server closes waiting socket.
9. Protocol whitelist rejects resize/kill/create and extra method fields.
10. IPC receiver excludes old output and preserves immediate output following response.
11. Cancelled request removes pending; dropping dedicated client closes named pipe reader.
12. Concurrent snapshot plus live output reconstruct exactly 1000 bytes, without omission/duplication.
13. 100KB in-flight named-pipe frame remains intact when atomic attach replaces the relay.

Initial WS integration tests correctly failed with MissingConnectionUpgradeHeader; cause was Hyper keep_alive(false), which rewrites the upgrade response. Keeping HTTP persistence enabled while enforcing a 10s outer connection deadline fixed all three failing WS tests. Early compilation also found and corrected async result/borrow/close API errors. Test declarations preceded implementation, but initial RED execution was compilation failure, not a fully isolated assertion failure; do not claim strict RED/GREEN evidence for every test.

## Safety / remaining integration

- No live user daemon restart, kill, or session writes were performed. IPC tests use random named-pipe names; HTTP/WS tests bind ephemeral loopback ports. Auth-negative/pending tests never invoke session APIs.
- Existing daemon versions do not understand AttachSessionAtomic. Mobile attach fails closed with a daemon-update/restart instruction; do not automatically restart a daemon holding user terminals.
- Assets were absent for the first successful library run, so explicit HTTP 503 fallback was verified. Task 2 must build dist-mobile, then rebuild/retest Rust to verify actual bundled mobile page.
- Physical Tailscale/mobile browser, QR scan, Korean input, real PTY output/resize preservation, full approved-session reconnect, and rendered desktop UI still require integration validation.
- Final tweaks after the 69-pass run: reject nonempty HTTP body headers, canonical port-80 Origin support; included in final all-targets run.
- No staging or commits. Source-only review required; target-mobile is generated output and must not be published.

## Reference consulted

Official Axum WebSocket upgrade/limit documentation: https://docs.rs/axum/latest/axum/extract/struct.WebSocketUpgrade.html
Official if-addrs API/package documentation: https://docs.rs/crate/if-addrs/0.14.0

## Final verification

`cargo test --all-targets --manifest-path src-tauri/Cargo.toml --target-dir target-mobile -j 3`

Session 57732 completed successfully (exit 0): library 69 passed/0 failed (24.42s), CLI 23 passed/0 failed (0.02s), GUI and daemon targets compiled and each ran 0 tests. This includes the final HTTP body rejection and port-80 normalization changes. Total 92 passing Rust tests. Missing-assets build warning remains expected until Task 2 generates dist-mobile.

`git diff --check -- src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs src-tauri/src/mobile_pairing src-tauri/src/ipc src-tauri/src/lib.rs src-tauri/src/pty/mod.rs` exited 0. Git only reported repository LF/CRLF conversion warnings.

Initial baseline session 43085 was queried with Ctrl+C and was already completed (exit 1, missing Pairings/base_url/valid_headers implementations from initial test-only source). No lingering baseline test session remains.
