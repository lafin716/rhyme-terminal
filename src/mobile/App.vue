<script setup lang="ts">
import appIcon from "../../src-tauri/icons/32x32.png";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watchEffect } from "vue";
import { Terminal } from "@xterm/xterm";
import { MobileClient } from "./client";
import { decodeBase64 } from "./encoding";
import { takeInvitation } from "./invitation";
import type { AttachResult, ClientState } from "./types";

const terminalHost = ref<HTMLElement | null>(null);
const composer = ref("");
const composing = ref(false);
const busy = ref(false);
const state = ref<ClientState>({
  phase: "disconnected", verification: null, error: null, sessions: [], attached: null,
});
let terminal: Terminal | null = null;
let terminalRenderGeneration = 0;

const invite = takeInvitation(window.location, url => window.history.replaceState(null, "", url));
const socketUrl = `${window.location.protocol === "https:" ? "wss:" : "ws:"}//${window.location.host}/ws`;
const client = new MobileClient({
  url: socketUrl,
  invite,
  deviceName: navigator.platform || "모바일 브라우저",
  storage: sessionStorage,
  createSocket: url => new WebSocket(url),
  onState: value => { state.value = value; },
  onOutput: data => terminal?.write(data),
  onAttach: showAttach,
});

const statusText = computed(() => ({
  connecting: "PC에 연결 중…",
  pending: "PC에서 연결 승인을 기다리고 있습니다.",
  authenticated: state.value.attached ? "터미널 연결됨" : "연결됨",
  reconnecting: "연결이 끊겨 다시 연결 중…",
  disconnected: "연결 끊김",
  error: "연결 오류",
})[state.value.phase]);
const inputEnabled = computed(() => state.value.phase === "authenticated" && !!state.value.attached && !busy.value);
watchEffect(() => {
  const enabled = inputEnabled.value;
  if (terminal) terminal.options.disableStdin = !enabled;
});

async function selectSession(event: Event) {
  const id = (event.target as HTMLSelectElement).value;
  if (!id) return;
  busy.value = true;
  try {
    await client.attach(id);
  } catch (error) {
    state.value = { ...state.value, error: error instanceof Error ? error.message : "터미널 연결에 실패했습니다." };
  } finally { busy.value = false; }
}

async function showAttach(result: AttachResult, isCurrent: () => boolean) {
  const generation = ++terminalRenderGeneration;
  terminal?.dispose();
  terminal = new Terminal({
    cols: result.info.cols,
    rows: result.info.rows,
    convertEol: false,
    cursorBlink: false,
    disableStdin: true,
    fontFamily: "Cascadia Mono, Consolas, monospace",
    fontSize: 14,
    scrollback: 10_000,
    theme: { background: "#090d12", foreground: "#dce7ef", cursor: "#82d3ff", selectionBackground: "#24516b" },
  });
  await nextTick();
  if (!terminalHost.value || generation !== terminalRenderGeneration || !isCurrent()) return;
  const cellWidth = 8.5;
  const cellHeight = 18;
  terminalHost.value.style.width = `${Math.max(result.info.cols * cellWidth + 24, 320)}px`;
  terminalHost.value.style.height = `${Math.max(result.info.rows * cellHeight + 16, 240)}px`;
  terminal.open(terminalHost.value);
  terminal.onData(data => {
    if (!inputEnabled.value || generation !== terminalRenderGeneration || !isCurrent()) return;
    // WebSocket preserves key order. Do not lock input while awaiting an ACK:
    // subsequent keystrokes must still be sent, and failures must not be replayed.
    void client.sendInput(data).catch(reportInputError);
  });
  terminal.write(decodeBase64(result.scrollback));
}

function reportInputError(error: unknown) {
  state.value = { ...state.value, error: error instanceof Error ? error.message : "입력 전송에 실패했습니다." };
}

async function transmit(value: string) {
  if (!value || !inputEnabled.value) return;
  busy.value = true;
  try { await client.sendInput(value); }
  catch (error) { reportInputError(error); }
  finally { busy.value = false; }
}

async function submitComposer() {
  if (composing.value || !composer.value) return;
  const value = composer.value;
  await transmit(`${value}\r`);
  if (!state.value.error) composer.value = "";
}

function onComposerKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && !event.shiftKey && !event.isComposing && !composing.value) {
    event.preventDefault();
    void submitComposer();
  }
}

function reloadPage() { window.location.reload(); }

onMounted(() => client.connect());
onBeforeUnmount(() => { client.disconnect(); terminal?.dispose(); });
</script>

<template>
  <main>
    <header>
      <div class="brand"><img :src="appIcon" alt="" /><div class="brand-text"><strong>rhyme-terminal</strong><span>모바일 터미널</span></div></div>
      <span class="status" :data-phase="state.phase">{{ statusText }}</span>
    </header>

    <section v-if="state.phase === 'pending'" class="pairing card">
      <p>PC의 승인 화면에 아래 확인 번호가 표시되는지 확인하세요.</p>
      <output>{{ state.verification }}</output>
      <small>이 페이지를 닫으면 승인 요청이 취소됩니다.</small>
    </section>

    <section v-else-if="state.phase === 'error' && !state.sessions.length" class="card error-card">
      <h1>연결할 수 없습니다</h1>
      <p>{{ state.error }}</p>
      <button type="button" @click="reloadPage">다시 시도</button>
    </section>

    <template v-else>
      <section class="toolbar">
        <label for="session">터미널</label>
        <select id="session" :disabled="state.phase !== 'authenticated' || busy" :value="state.attached?.id ?? ''" @change="selectSession">
          <option value="" disabled>{{ state.sessions.length ? "터미널을 선택하세요" : "실행 중인 터미널이 없습니다" }}</option>
          <option v-for="session in state.sessions" :key="session.id" :value="session.id">{{ session.name }}</option>
        </select>
        <button type="button" :disabled="state.phase !== 'authenticated'" aria-label="목록 새로 고침" @click="client.refreshSessions()">↻</button>
      </section>

      <p v-if="state.error" class="inline-error">{{ state.error }}</p>
      <section class="terminal-scroll" aria-label="터미널 출력">
        <div v-if="!state.attached" class="terminal-placeholder">터미널을 선택하면 PC 화면 크기 그대로 표시됩니다.</div>
        <div ref="terminalHost" class="terminal-host"></div>
      </section>

      <section class="input-panel">
        <textarea
          v-model="composer"
          rows="2"
          placeholder="한글을 조합한 뒤 Enter 또는 전송"
          :disabled="!inputEnabled"
          @compositionstart="composing = true"
          @compositionend="composing = false"
          @keydown="onComposerKeydown"
        ></textarea>
        <button class="send" type="button" :disabled="!inputEnabled || !composer" @click="submitComposer">전송</button>
        <div class="keys" aria-label="보조 키">
          <button type="button" :disabled="!inputEnabled" @click="transmit('\r')">Enter</button>
          <button type="button" :disabled="!inputEnabled" @click="transmit('\t')">Tab</button>
          <button type="button" :disabled="!inputEnabled" @click="transmit('\u001b')">Esc</button>
          <button type="button" :disabled="!inputEnabled" @click="transmit('\u0003')">Ctrl+C</button>
        </div>
      </section>
    </template>
  </main>
</template>
