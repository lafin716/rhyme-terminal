import { readFileSync, writeFileSync } from "node:fs";
const path = "src-tauri/gen/android/app/src/main/AndroidManifest.xml";
let manifest = readFileSync(path, "utf8");
if (manifest.includes("android:usesCleartextTraffic=")) {
  manifest = manifest.replace(/android:usesCleartextTraffic="[^"]*"/, 'android:usesCleartextTraffic="true"');
} else {
  manifest = manifest.replace("<application", '<application android:usesCleartextTraffic="true"');
}
// The gateway intentionally uses direct LAN/Tailscale HTTP and WebSocket.
writeFileSync(path, manifest);
