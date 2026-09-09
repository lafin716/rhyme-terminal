<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { Icon } from '@iconify/vue';
import { useLoopRouting } from '../composables/useLoopRouting';
import { useSettings } from '../composables/useSettings';
import type { LoopConversation, LoopGroup, LoopStart } from '../lib/loop-routing';
const props = defineProps<{ workspaceId: string; workspaceIndex: number; cwd: string; name: string }>();
const emit = defineEmits<{ close: []; created: [group: LoopGroup] }>();
const loops = useLoopRouting();
const requestId = crypto.randomUUID();
const dialog = ref<HTMLDialogElement>();
const mode = ref<'prompt' | 'session'>('prompt');
const name = ref(props.name);
const prompt = ref('');
const query = ref('');
const sessions = ref<LoopConversation[]>([]);
const selected = ref<LoopConversation>();
const loading = ref(false);
const loaded = ref(false);
const busy = ref(false);
const error = ref('');
const listError = ref('');
const candidates = computed(() => loops.state.settings.candidates.filter(c => c.enabled));
const filtered = computed(() => sessions.value.filter(s => `${s.title} ${s.agent} ${s.label} ${s.sessionId}`.toLocaleLowerCase().includes(query.value.toLocaleLowerCase())));
const valid = computed(() => name.value.trim().length > 0 && candidates.value.length > 0 && (mode.value === 'prompt' ? prompt.value.trim().length > 0 : !!selected.value));
onMounted(() => dialog.value?.showModal());
function close() { if (!busy.value) emit('close'); }
async function load() {
  loading.value = true; listError.value = ''; selected.value = undefined;
  try { sessions.value = await loops.conversations(props.cwd); loaded.value = true; }
  catch (e) { listError.value = String(e); }
  finally { loading.value = false; }
}
function changeMode(value: 'prompt' | 'session') {
  mode.value = value; error.value = '';
  if (value === 'session' && !loaded.value && !loading.value) void load();
}
async function start() {
  if (busy.value || !valid.value) return;
  busy.value = true; error.value = '';
  const start: LoopStart = mode.value === 'prompt' ? { kind: 'prompt', prompt: prompt.value.trim() }
    : { kind: 'session', sessionId: selected.value!.sessionId, candidateKey: selected.value!.candidateKey };
  try { emit('created', await loops.create(name.value.trim(), props.workspaceId, props.workspaceIndex, props.cwd, start, requestId)); }
  catch (e) { error.value = String(e); }
  finally { busy.value = false; }
}
function settings() { close(); useSettings().openSettings(); }
const date = (value: number) => new Intl.DateTimeFormat('ko-KR', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(value);
</script>

<template>
  <Teleport to="body">
    <dialog ref="dialog" class="loop-create" aria-labelledby="loop-create-title" @cancel.prevent="close" @keydown.escape.stop>
      <form @submit.prevent="start">
        <header>
          <div><p class="eyebrow">에이전트 루프</p><h2 id="loop-create-title">어떤 작업을 시작할까요?</h2><p class="intro">새 작업을 입력하거나, 이어갈 대화를 직접 선택하세요.</p></div>
          <button class="icon-button" type="button" aria-label="닫기" :disabled="busy" @click="close"><Icon icon="lucide:x" width="20" /></button>
        </header>
        <div class="body">
          <div class="workspace"><Icon icon="lucide:folder" width="16" /><span>{{ cwd }}</span></div>
          <label class="field">루프 이름<input v-model="name" maxlength="256" :disabled="busy" required /></label>
          <fieldset class="mode" :disabled="busy">
            <legend class="sr-only">시작 방식</legend>
            <label :class="{ chosen: mode === 'prompt' }"><input type="radio" name="loop-start" value="prompt" :checked="mode === 'prompt'" @change="changeMode('prompt')" /><Icon icon="lucide:square-pen" width="19" /><span><strong>새 프롬프트</strong><small>새 대화에서 작업 시작</small></span></label>
            <label :class="{ chosen: mode === 'session' }"><input type="radio" name="loop-start" value="session" :checked="mode === 'session'" @change="changeMode('session')" /><Icon icon="lucide:messages-square" width="19" /><span><strong>기존 대화 이어가기</strong><small>이 폴더의 대화 직접 선택</small></span></label>
          </fieldset>
          <div v-if="mode === 'prompt'" class="prompt-panel">
            <label class="field" for="loop-prompt">시작할 작업<textarea id="loop-prompt" v-model="prompt" :disabled="busy" maxlength="16000" rows="6" placeholder="무엇을 만들거나 수정할까요? 목표와 완료 조건을 적어주세요." autofocus /></label>
            <p class="hint">입력한 내용으로 새 대화를 시작합니다. 기존 대화는 연결하지 않습니다.</p>
          </div>
          <section v-else class="session-panel" aria-label="기존 대화 선택" :aria-busy="loading">
            <div class="list-tools"><input v-model="query" type="search" placeholder="작업 내용, 에이전트, 계정 검색" aria-label="대화 검색" :disabled="busy" /><button type="button" :disabled="busy || loading" @click="load">새로고침</button></div>
            <p class="hint">루프에 참여하는 계정의 최근 대화입니다. 선택 후 아래 버튼을 눌러야 시작합니다.</p>
            <p v-if="loading" class="empty" role="status">대화를 불러오는 중…</p>
            <p v-else-if="listError" class="error" role="alert">{{ listError }}</p>
            <div v-else-if="!sessions.length" class="empty"><Icon icon="lucide:messages-square" width="28" /><strong>이 폴더에 이어갈 대화가 없습니다</strong><span>새 프롬프트로 작업을 시작하거나 참여 계정을 확인하세요.</span><button type="button" @click="changeMode('prompt')">새 프롬프트 작성</button></div>
            <p v-else-if="!filtered.length" class="empty">검색 결과가 없습니다. 다른 검색어를 입력하세요.</p>
            <fieldset v-else class="session-list" :disabled="busy"><legend class="sr-only">이어갈 대화</legend>
              <label v-for="session in filtered" :key="session.candidateKey + session.sessionId" class="session" :class="{ chosen: selected === session }">
                <input v-model="selected" type="radio" name="loop-session" :value="session" />
                <span class="session-copy"><strong>{{ session.title }}</strong><span class="metadata"><span>{{ session.agent === 'claude' ? 'Claude' : 'Codex' }} · {{ session.label }}</span><time :datetime="new Date(session.updatedAt).toISOString()">{{ date(session.updatedAt) }}</time></span><small>{{ session.sessionId.slice(0, 8) }}</small></span>
              </label>
            </fieldset>
            <p v-if="selected" class="selection" role="status">선택됨 · {{ selected.title }}</p>
          </section>
          <div v-if="!candidates.length" class="notice"><span>실행할 계정을 먼저 선택해주세요.</span><button type="button" :disabled="busy" @click="settings">루프 설정 열기</button></div>
          <p v-else class="routing"><Icon icon="lucide:repeat-2" width="15" />참여 계정 {{ candidates.length }}개 · 사용량에 따라 자동 전환 · automode</p>
          <p v-if="error" class="error" role="alert">{{ error }}</p>
        </div>
        <footer><span>{{ mode === 'prompt' ? '프롬프트를 입력한 후 시작하세요.' : selected ? '선택한 대화의 기록을 이어갑니다.' : '이어갈 대화를 하나 선택하세요.' }}</span><button type="button" :disabled="busy" @click="close">취소</button><button class="primary" type="submit" :disabled="busy || loading || !valid">{{ busy ? '시작 준비 중…' : mode === 'prompt' ? '새 작업 시작' : '선택한 대화 이어가기' }}</button></footer>
      </form>
    </dialog>
  </Teleport>
</template>

<style scoped>
.loop-create{--accent:#4ec9b0;--muted:#b0b5bb;--border:#41464d;width:min(680px,calc(100vw - 32px));max-height:calc(100dvh - 72px);padding:0;border:1px solid var(--border);border-radius:12px;background:#202225;color:#e6e8eb;box-shadow:0 24px 80px #0009;overflow:hidden;font-size:13px}
.loop-create::backdrop{background:#0009}form{display:flex;flex-direction:column;max-height:calc(100dvh - 74px)}header{display:flex;align-items:flex-start;justify-content:space-between;padding:24px 26px 18px;border-bottom:1px solid var(--border)}h2{font-size:21px;line-height:1.35;margin:5px 0 8px;font-weight:650}.eyebrow{color:var(--accent);font-size:12px;margin:0}.intro,.hint{color:var(--muted);line-height:1.6;margin:0}.body{padding:20px 26px;display:flex;flex-direction:column;gap:18px;overflow-y:auto;min-height:0}.workspace{display:flex;gap:8px;color:var(--muted);align-items:flex-start}.workspace span{overflow-wrap:anywhere}.workspace svg{flex-shrink:0}.field{display:flex;flex-direction:column;gap:8px;font-weight:600}input:not([type=radio]),textarea{font:inherit;background:#17191c;color:#eee;border:1px solid var(--border);border-radius:6px;padding:10px 12px;min-width:0}textarea{resize:vertical;min-height:120px;max-height:300px;line-height:1.7;font-weight:400}button{font:inherit;min-height:38px;padding:8px 12px;border:1px solid var(--border);border-radius:6px;background:#30343a;color:#eee;cursor:pointer}button:hover:not(:disabled){background:#3b4148}button:disabled{opacity:.45;cursor:default}:is(button,input,textarea):focus-visible{outline:2px solid var(--accent);outline-offset:3px}.icon-button{padding:8px;display:flex;align-items:center;justify-content:center;background:transparent;flex-shrink:0}.mode{display:grid;grid-template-columns:1fr 1fr;gap:12px;border:0;margin:0;padding:0}.mode label{display:flex;align-items:center;gap:10px;padding:14px 12px;background:#272b30;border:1px solid var(--border);border-radius:8px;cursor:pointer}.mode span{display:flex;flex-direction:column;gap:5px}.mode small{color:var(--muted);font-size:12px}.mode input,.session input{accent-color:var(--accent);flex-shrink:0}.chosen{border-color:var(--accent)!important;background:#263b37!important}.hint{font-size:12px;margin-top:8px}.list-tools{display:flex;gap:8px}.list-tools input{flex:1}.session-list{max-height:260px;overflow-y:auto;border:0;padding:3px;margin:12px -3px 0;display:flex;flex-direction:column;gap:8px}.session{display:flex;align-items:flex-start;gap:10px;padding:12px;background:#272b30;border:1px solid var(--border);border-radius:7px;cursor:pointer}.session-copy{display:flex;flex-direction:column;gap:8px;min-width:0;flex:1}.session-copy strong{font-size:13px;font-weight:500;line-height:1.55;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;overflow-wrap:anywhere}.metadata{display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;color:var(--muted);font-size:12px}.session-copy small{color:var(--muted);font-family:monospace}.empty{display:flex;flex-direction:column;align-items:center;text-align:center;gap:10px;padding:28px 16px;color:var(--muted);line-height:1.6;background:#191c20;border-radius:8px;margin-top:12px}.empty strong{color:#e6e8eb}.empty span{font-size:12px}.selection{color:var(--accent);font-size:12px;line-height:1.5;overflow-wrap:anywhere;max-height:60px;overflow:auto}.routing{display:flex;align-items:center;gap:7px;color:var(--muted);font-size:12px;margin:0}.notice{display:flex;justify-content:space-between;align-items:center;gap:10px;color:#f3ce8a}.error{color:#ffb3ad;line-height:1.6;overflow-wrap:anywhere;margin:0}footer{display:flex;align-items:center;gap:8px;padding:16px 26px;background:#262a2e;border-top:1px solid var(--border)}footer>span{flex:1;color:var(--muted);font-size:12px;line-height:1.5}.primary{background:var(--accent);color:#112a24;border-color:var(--accent);font-weight:650}.primary:hover:not(:disabled){background:#6cddc5}.sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%)}@media(max-width:560px){header,.body{padding:18px}h2{font-size:19px}.mode{grid-template-columns:1fr}footer{padding:14px 18px;flex-wrap:wrap}footer>span{flex-basis:100%}.routing{flex-wrap:wrap}footer .primary{flex:1}}
</style>
