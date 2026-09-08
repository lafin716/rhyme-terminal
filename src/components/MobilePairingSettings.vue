<script setup lang="ts">
import { ref, watch } from "vue";
import { t } from "../composables/useI18n";
import { useMobilePairing } from "../composables/useMobilePairing";
import { validPort } from "../lib/mobile-pairing";
const { status, interfaces, ip, port, busy, ready, error, selectedAvailable, canStart, inviteSeconds, qrSource, refreshInterfaces, start, stop, invite, approve, revoke } = useMobilePairing();
const copied = ref<"server" | "invite" | null>(null);
watch(() => [status.value.url, status.value.invite?.url], () => { copied.value = null; });
async function copyUrl(kind: "server" | "invite") {
  const url = kind === "invite" ? status.value.invite?.url : status.value.url;
  if (!url || (kind === "invite" && inviteSeconds.value <= 0)) return;
  try { await navigator.clipboard.writeText(url); copied.value = kind; }
  catch { error.value = t("Could not copy the address. Select and copy it manually."); }
}</script>

<template>
  <section class="mobile-settings" aria-labelledby="mobile-pairing-title">
    <header>
      <h2 id="mobile-pairing-title">{{ t('Mobile connection') }}</h2>
      <p>{{ t('Pair a mobile browser to view and control existing terminals.') }}</p>
    </header>
    <p class="notice">{{ t('Connect both devices through Tailscale. Direct LAN connections use HTTP without TLS.') }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <div class="card">
      <div class="card-heading"><h3>{{ t('Connection address') }}</h3><span class="state" :class="{ running: status.running }">{{ status.running ? t('Running') : t('Stopped') }}</span></div>
      <label for="mobile-interface">{{ t('Network interface / IP') }}</label>
      <div class="row">
        <select id="mobile-interface" v-model="ip" :disabled="status.running || busy || !ready">
          <option v-if="!ip" value="" disabled>{{ t('Select an IP address') }}</option>
          <option v-if="ip && !selectedAvailable" :value="ip">{{ ip }} — {{ t('Unavailable') }}</option>
          <option v-for="item in interfaces" :key="`${item.name}:${item.ip}`" :value="item.ip">{{ item.name }} — {{ item.ip }}{{ item.isTailscale ? ' (Tailscale)' : '' }}</option>
        </select>
        <button type="button" :disabled="busy" @click="refreshInterfaces">{{ t('Refresh') }}</button>
      </div>
      <p v-if="ready && ip && !selectedAvailable && !status.running" class="error">{{ t('The selected IP is no longer available. Refresh and choose an address explicitly.') }}</p>
      <p v-if="ready && !interfaces.length" class="hint">{{ t('No usable network address was found. Check your network and refresh.') }}</p>
      <label for="mobile-port">{{ t('Port') }}</label>
      <p v-if="!status.running && !validPort(port)" class="error">{{ t('Enter a whole-number port from 1 to 65535.') }}</p>
      <input id="mobile-port" v-model.number="port" type="number" min="1" max="65535" step="1" :disabled="status.running || busy || !ready" />
      <div class="row actions">
        <button v-if="!status.running" type="button" class="primary" :disabled="!canStart" @click="start">{{ t('Start mobile connection') }}</button>
        <button v-else type="button" class="danger" :disabled="busy" @click="stop">{{ t('Stop mobile connection') }}</button>
      </div>
      <p class="hint">{{ t('Stop the server before changing its address. Hiding the app keeps the connection running; exiting clears all paired devices.') }}</p>
      <template v-if="status.url">
        <label for="mobile-url">{{ t('Server address') }}</label>
        <div class="row"><input id="mobile-url" :value="status.url" readonly @focus="($event.target as HTMLInputElement).select()" /><button type="button" @click="copyUrl('server')">{{ copied === 'server' ? t('Copied') : t('Copy address') }}</button></div>
      </template>
    </div>
    <div v-if="status.running" class="card">
      <div class="card-heading"><h3>{{ t('Pair a new device') }}</h3><button type="button" :disabled="busy" @click="invite">{{ t('Generate new QR') }}</button></div>
      <div v-if="qrSource" class="invite">
        <img :src="qrSource" width="208" height="208" :alt="t('Mobile pairing QR code')" />
        <div><button type="button" @click="copyUrl('invite')">{{ copied === 'invite' ? t('Copied') : t('Copy invitation link') }}</button><p>{{ t('Scan this QR with your phone camera, then compare the verification code before approving.') }}</p><p class="countdown" aria-live="off">{{ t('Expires in {seconds} seconds', { seconds: inviteSeconds }) }}</p></div>
      </div>
      <p v-else class="hint">{{ t('The invitation was used or expired. Generate a new QR to pair another device.') }}</p>
    </div>
    <div v-if="status.running" class="card">
      <h3>{{ t('Waiting for approval') }}</h3>
      <p v-if="!status.pending.length" class="hint">{{ t('No devices are waiting for approval.') }}</p>
      <div v-for="request in status.pending" :key="request.requestId" class="device">
        <div class="device-text"><strong>{{ request.name }}</strong><p>{{ t('Verification code') }}: <code>{{ request.verification }}</code></p></div>
        <div class="row"><button type="button" :disabled="busy" @click="approve(request.requestId, false)">{{ t('Reject') }}</button><button type="button" class="primary" :disabled="busy" @click="approve(request.requestId, true)">{{ t('Approve') }}</button></div>
      </div>
    </div>
    <div v-if="status.running" class="card">
      <h3>{{ t('Paired devices') }}</h3>
      <p v-if="!status.devices.length" class="hint">{{ t('No paired devices.') }}</p>
      <div v-for="device in status.devices" :key="device.deviceId" class="device"><div class="device-text"><strong>{{ device.name }}</strong><p>{{ device.connected ? t('Connected') : t('Disconnected') }}</p></div><button type="button" class="danger" :disabled="busy" @click="revoke(device.deviceId)">{{ t('Revoke access') }}</button></div>
    </div>
  </section>
</template>

<style scoped>
.mobile-settings { display: flex; flex-direction: column; gap: 18px; min-width: 0; color: #ddd; }
h2 { margin: 0 0 8px; font-size: 23px; color: #f1f1f1; } h3 { margin: 0; font-size: 15px; color: #eee; }
p { margin: 8px 0; line-height: 1.6; font-size: 13px; }
header p, .hint, .device p { color: #aaa; }
.card { background: #202020; border: 1px solid #333; border-radius: 10px; padding: 20px; min-width: 0; }
.card-heading, .device { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
.card-heading { margin-bottom: 18px; }
label { display: block; font-size: 13px; margin: 15px 0 8px; }
.row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
select, input { box-sizing: border-box; min-width: 0; background: #161616; color: #eee; border: 1px solid #444; border-radius: 6px; padding: 10px 12px; font: inherit; font-size: 13px; }
.row select, .row input { flex: 1 1 220px; width: 100%; }
#mobile-port { width: 140px; }
button { background: #303030; border: 1px solid #4a4a4a; border-radius: 6px; padding: 9px 13px; color: #eee; cursor: pointer; font: inherit; font-size: 13px; }
button:hover:not(:disabled) { background: #3b3b3b; }
button:disabled, select:disabled, input:disabled { opacity: .5; cursor: default; }
button.primary { background: #3261a8; border-color: #527eba; } button.danger { color: #ffaaaa; }
button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid #7bb3ff; outline-offset: 2px; }
.actions { margin-top: 18px; }
.notice { padding: 12px 16px; border-left: 3px solid #b6a16c; background: #2a2822; color: #d9d0b9; }
.error { color: #ffb1b1; overflow-wrap: anywhere; }
.state { font-size: 12px; color: #aaa; } .state.running { color: #97d8b0; }
.invite { display: flex; gap: 24px; align-items: center; flex-wrap: wrap; }.invite img { background: white; border-radius: 8px; max-width: 100%; height: auto; }.invite > div { flex: 1 1 220px; }
.countdown { color: #b7d6ff; font-variant-numeric: tabular-nums; }
.device { padding: 16px 0; border-bottom: 1px solid #333; }.device:last-child { border-bottom: 0; padding-bottom: 0; }.device-text { min-width: 0; overflow-wrap: anywhere; }
code { font-family: Consolas, monospace; font-size: 19px; color: #bed9ff; letter-spacing: 3px; }
</style>
