# Mobile pairing interface contract

All JSON uses camelCase except daemon SessionInfo/Event payloads, which retain existing serde field names.

## Tauri commands

`mobile_pairing_interfaces() -> NetworkInterface[]`
`mobile_pairing_status() -> PairingStatus`
`mobile_pairing_start({ip: string, port: number}) -> PairingStatus`
`mobile_pairing_stop() -> PairingStatus`
`mobile_pairing_invite() -> PairingStatus`
`mobile_pairing_approve({requestId: string, approve: boolean}) -> PairingStatus`
`mobile_pairing_revoke({deviceId: string}) -> PairingStatus`

```ts
interface NetworkInterface { name: string; ip: string; isTailscale: boolean }
interface PendingDevice { requestId: string; name: string; verification: string }
interface PairedDevice { deviceId: string; name: string; connected: boolean }
interface PairingStatus {
  running: boolean; ip: string | null; port: number | null; url: string | null;
  invite: { url: string; qrSvg: string; expiresAt: number } | null;
  pending: PendingDevice[]; devices: PairedDevice[];
}
```

expiresAt is Unix epoch milliseconds. `url` is base `http://IP:port/`; invite URL is base plus `#invite=TOKEN`. First start creates invite. `qrSvg` is server-generated SVG safe to show using an encoded image data URI.

## WebSocket endpoint `/ws`

First client message is one of:
```json
{"type":"pair","invite":"token","name":"device name"}
{"type":"auth","token":"device token"}
```
Server replies:
```json
{"type":"pending","verification":"123456"}
{"type":"authenticated","token":"device token","deviceId":"uuid"}
{"type":"error","message":"reason"}
```
Pairing rejection/expiry/revocation/authentication failure sends error when possible and closes socket. Reconnect authentication also returns authenticated with same token.

After `pending`, the same socket remains open. Pending request IDs are bound to that socket's lifetime. Desktop approval pushes `authenticated` on that socket and removes its pending record. Rejection, expiry and socket close remove pending state. HTTP navigation/assets require exact Host but may omit Origin; WebSocket upgrades require both exact Host and expected Origin.

Authenticated client sends:
```json
{"type":"list","requestId":1}
{"type":"attach","requestId":2,"id":"session uuid"}
{"type":"detach","requestId":3,"id":"session uuid"}
{"type":"input","requestId":4,"id":"session uuid","data":"base64 utf8"}
```
Responses:
```json
{"type":"result","requestId":1,"data":[]}
{"type":"result","requestId":2,"data":{"info":{},"scrollback":"base64"}}
{"type":"result","requestId":4,"data":null}
{"type":"error","requestId":4,"message":"reason"}
{"type":"event","data":{"event":"pty_output","id":"uuid","data":"base64"}}
```
List returns SessionInfo[]. Attach returns existing AttachResult. Only one attached session per mobile socket; switching detaches old one. Live output for new session is delivered after attach result. No resize/create/kill/file APIs exposed. Input requires matching attached id. Mobile clears terminal on attach result and uses returned cols/rows. On transport loss disable input, never replay it. Requests have client timeout; stale callbacks cannot change a newer socket.

The IPC snapshot/subscription barrier ensures snapshot bytes and subsequent output join exactly once. PTY scrollback append and output publication share the barrier lock. Any lag in mobile's daemon/event/WebSocket path closes the connection for resynchronization. Timed-out daemon requests clean pending state or close the dedicated client.

## Assets

Task 2 produces `dist-mobile/mobile.html` and `dist-mobile/assets/*`, relative asset URLs. Backend serves mobile.html at `/`; unknown paths 404. Task 1 chooses embedding crate/build integration and informs controller. Mobile script can use existing xterm/Vue dependencies; QR generated in Rust to avoid frontend dependency coordination.
`session_added`, `session_removed`, `session_renamed` daemon events are also forwarded after authentication to update the already-authorized session list. Agent activity/status events are not forwarded.
