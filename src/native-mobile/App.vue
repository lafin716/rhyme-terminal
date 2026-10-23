<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import icon from "../../src-tauri/icons/128x128.png";
import {
  MAX_NAME_LENGTH, connectWithInvitation, loadRemotes, remoteTarget, removeRemote, renameRemote, saveRemotes, touchRemote,
  type RemoteEntry,
} from "./remotes";

const remotes = ref<RemoteEntry[]>(loadRemotes(localStorage));
const invitation = ref("");
const error = ref("");
const adding = ref(remotes.value.length === 0);
const editing = ref<string | null>(null);
const draftName = ref("");
const removing = ref<string | null>(null);
const leaving = ref(false);
const empty = computed(() => remotes.value.length === 0);
const formatter = new Intl.DateTimeFormat("ko-KR", { dateStyle: "medium", timeStyle: "short" });

function persist(next: RemoteEntry[]) {
  remotes.value = next;
  try { saveRemotes(localStorage, next); } catch { error.value = "연결 목록을 저장하지 못했습니다."; }
}

let leaveTimer: ReturnType<typeof setTimeout> | undefined;

function navigate(target: string) {
  leaving.value = true;
  // Re-enable the screen if navigation never happens (blocked or silently failed).
  leaveTimer = setTimeout(() => { leaving.value = false; }, 10_000);
  window.location.assign(target);
}

// Returning with Back restores this page from bfcache with stale state: reset it.
function onPageShow(event: PageTransitionEvent) {
  if (!event.persisted) return;
  clearTimeout(leaveTimer);
  leaving.value = false;
  editing.value = null;
  removing.value = null;
  remotes.value = loadRemotes(localStorage);
  adding.value = remotes.value.length === 0;
}
onMounted(() => window.addEventListener("pageshow", onPageShow));
onBeforeUnmount(() => { window.removeEventListener("pageshow", onPageShow); clearTimeout(leaveTimer); });

function connectInvitation() {
  try {
    const result = connectWithInvitation(remotes.value, invitation.value, Date.now());
    invitation.value = "";
    error.value = "";
    persist(result.remotes);
    navigate(result.target);
  } catch (value) { error.value = value instanceof Error ? value.message : "초대 링크를 확인하세요."; }
}

function connectSaved(entry: RemoteEntry) {
  error.value = "";
  persist(touchRemote(remotes.value, entry.origin, Date.now()));
  navigate(remoteTarget(entry.origin));
}

async function startRename(entry: RemoteEntry) {
  removing.value = null;
  editing.value = entry.origin;
  draftName.value = entry.name;
  await nextTick();
  document.getElementById(`rename-${entry.origin}`)?.focus();
}

function saveRename(entry: RemoteEntry) {
  persist(renameRemote(remotes.value, entry.origin, draftName.value));
  editing.value = null;
}

function confirmRemove(entry: RemoteEntry) {
  persist(removeRemote(remotes.value, entry.origin));
  removing.value = null;
  if (!remotes.value.length) adding.value = true;
}

function lastConnected(entry: RemoteEntry) {
  return entry.lastConnectedAt === null ? "연결 기록 없음" : `마지막 연결 ${formatter.format(entry.lastConnectedAt)}`;
}
</script>

<template>
  <main>
    <header>
      <img :src="icon" alt="" width="40" height="40" />
      <div>
        <h1>Rhyme Terminal</h1>
        <p>원격 PC 연결</p>
      </div>
    </header>

    <p class="notice">이 앱에는 자체 터미널이 없습니다. 아래 목록에서 PC에 연결해야 터미널을 사용할 수 있습니다.</p>

    <section aria-labelledby="remotes-title">
      <div class="section-head">
        <h2 id="remotes-title">원격 연결</h2>
        <button v-if="!empty && !adding" type="button" class="ghost" :disabled="leaving" @click="adding = true">+ 새 PC</button>
      </div>

      <p v-if="empty" class="empty">저장된 원격 PC가 없습니다. PC 초대 링크로 첫 연결을 추가하세요.</p>
      <ul v-else class="remotes">
        <li v-for="entry in remotes" :key="entry.origin" class="remote">
          <form v-if="editing === entry.origin" class="rename" @submit.prevent="saveRename(entry)">
            <label :for="`rename-${entry.origin}`">연결 이름</label>
            <input :id="`rename-${entry.origin}`" v-model="draftName" :maxlength="MAX_NAME_LENGTH" autocomplete="off" />
            <div class="row">
              <button type="submit">저장</button>
              <button type="button" class="ghost" @click="editing = null">취소</button>
            </div>
          </form>
          <template v-else>
            <div class="remote-info">
              <strong>{{ entry.name }}</strong>
              <span class="origin">{{ entry.origin }}</span>
              <span class="meta">{{ lastConnected(entry) }}</span>
            </div>
            <div v-if="removing === entry.origin" class="row confirm" role="group" :aria-label="`${entry.name} 삭제 확인`">
              <span>목록에서 삭제할까요?</span>
              <button type="button" class="danger" @click="confirmRemove(entry)">삭제</button>
              <button type="button" class="ghost" @click="removing = null">취소</button>
            </div>
            <div v-else class="row">
              <button type="button" class="connect" :disabled="leaving" @click="connectSaved(entry)">연결</button>
              <button type="button" class="ghost" :disabled="leaving" @click="startRename(entry)">이름 변경</button>
              <button type="button" class="ghost" :disabled="leaving" @click="removing = entry.origin; editing = null">삭제</button>
            </div>
          </template>
        </li>
      </ul>
      <small v-if="!empty" class="hint">PC 앱을 다시 시작했거나 연결이 거부되면 새 초대 링크로 연결하세요.</small>
    </section>

    <section v-if="adding" class="add" aria-labelledby="add-title">
      <h2 id="add-title">새 PC 연결</h2>
      <ol>
        <li>PC 앱의 모바일 연결을 켭니다.</li>
        <li>같은 네트워크 또는 Tailscale에 연결합니다.</li>
        <li>초대 링크를 붙여넣고 PC에서 연결을 승인합니다.</li>
      </ol>
      <form @submit.prevent="connectInvitation">
        <label for="invitation">PC 초대 링크</label>
        <textarea id="invitation" v-model="invitation" rows="3" placeholder="http://…/#invite=…" autocomplete="off" autocapitalize="off" spellcheck="false"></textarea>
        <p v-if="error" role="alert" class="error">{{ error }}</p>
        <div class="row">
          <button type="submit" class="connect" :disabled="!invitation.trim() || leaving">초대 링크로 연결</button>
          <button v-if="!empty" type="button" class="ghost" @click="adding = false; invitation = ''; error = ''">닫기</button>
        </div>
      </form>
    </section>
    <p v-else-if="error" role="alert" class="error">{{ error }}</p>

    <small class="footnote">초대 링크와 인증 정보는 저장하지 않으며, 저장되는 것은 PC 주소와 이름뿐입니다. PC의 터미널은 연결을 끊어도 계속 실행됩니다.</small>
  </main>
</template>

<style>
:root { font-family: system-ui, sans-serif; color: #dce7ef; background: #090d12; color-scheme: dark; }
* { box-sizing: border-box; }
body { margin: 0; }
main { width: min(100%, 560px); margin: auto; padding: max(28px, env(safe-area-inset-top)) 20px max(28px, env(safe-area-inset-bottom)); }
header { display: flex; align-items: center; gap: 12px; }
h1 { margin: 0; font-size: 22px; }
header p { margin: 0; color: #a5b4c4; font-size: 14px; }
h2 { margin: 0; font-size: 17px; }
p, li, small { color: #a5b4c4; line-height: 1.6; }
.notice { margin: 20px 0; padding: 12px 14px; border-radius: 10px; background: #111923; border: 1px solid #22303f; font-size: 14px; }
section { margin-top: 24px; }
.section-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 44px; }
.empty { margin: 12px 0 0; }
.remotes { list-style: none; margin: 12px 0 0; padding: 0; display: grid; gap: 12px; }
.remote { padding: 14px; border: 1px solid #263445; border-radius: 12px; background: #0f161e; }
.remote-info { display: grid; gap: 2px; min-width: 0; }
.remote-info strong { font-size: 16px; color: #e8f1f8; overflow-wrap: anywhere; }
.origin { font-family: ui-monospace, monospace; font-size: 13px; color: #82d3ff; overflow-wrap: anywhere; }
.meta { font-size: 12px; color: #8495a7; }
.row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 12px; }
.confirm span { flex: 1 1 100%; color: #ffd2cc; font-size: 14px; }
.hint { display: block; margin-top: 12px; font-size: 12px; }
ol { padding-left: 22px; margin: 12px 0 20px; }
li { margin: 6px 0; }
label { display: block; margin-bottom: 8px; font-weight: 600; }
textarea, input { width: 100%; padding: 12px; border: 1px solid #344456; border-radius: 10px; background: #111923; color: inherit; font: inherit; }
textarea { resize: vertical; }
textarea:focus, input:focus, button:focus-visible { outline: 2px solid #82d3ff; outline-offset: 2px; }
button { min-height: 44px; border: 1px solid transparent; border-radius: 10px; padding: 10px 16px; font: inherit; font-weight: 600; background: #82d3ff; color: #09131d; }
button.connect { flex: 1 1 auto; }
button.ghost { background: transparent; color: #cfe2f0; border-color: #344456; }
button.danger { background: #ffb4ab; color: #2b0b07; }
button:disabled { opacity: .45; }
.add form .row button.connect { flex: 1 1 100%; }
.error { color: #ffb4ab; }
.footnote { display: block; margin-top: 28px; font-size: 12px; }
</style>
