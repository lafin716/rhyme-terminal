<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { Icon } from '@iconify/vue';
import { useLoopRouting } from '../composables/useLoopRouting';
import type { LoopGroup, LoopSettings } from '../lib/loop-routing';
const props = defineProps<{ workspaceId: string; workspaceIndex: number; cwd: string; name: string }>();
const emit = defineEmits<{ close: []; created: [group: LoopGroup] }>();
const loops = useLoopRouting();
const requestId = crypto.randomUUID();
const dialog = ref<HTMLDialogElement>();
const name = ref(props.name);
const key = (c: { agent: string; profileId: string | null }) => `${c.agent}:${c.profileId ?? 'system'}`;
const participants = ref(loops.state.settings.candidates.filter(c => c.enabled).map(key));
const threshold = ref(loops.state.settings.shortThreshold);
const strategy = ref<LoopSettings['strategy']>(loops.state.settings.strategy ?? 'SMART');
const polling = ref(loops.state.settings.pollingIntervalSeconds ?? 30);
const interrupt = ref(loops.state.settings.interruptTimeoutSeconds ?? 5);
const autoResume = ref(loops.state.settings.autoResume ?? true);
const busy = ref(false);
const error = ref('');
const valid = computed(() => !!name.value.trim() && participants.value.length > 0 && threshold.value >= 1 && threshold.value <= 100);
onMounted(() => dialog.value?.showModal());
function close() { if (!busy.value) emit('close'); }
async function create() {
  if (busy.value || !valid.value) return;
  busy.value = true; error.value = '';
  try {
    const settings = { ...loops.state.settings, shortThreshold: threshold.value, weeklyThreshold: threshold.value, strategy: strategy.value, pollingIntervalSeconds: polling.value, interruptTimeoutSeconds: interrupt.value, autoResume: autoResume.value,
      candidates: loops.state.settings.candidates.map(c => ({ ...c, enabled: c.enabled || participants.value.includes(key(c)) })) };
    emit('created', await loops.create({ name: name.value.trim(), workspaceId: props.workspaceId, workspaceIndex: props.workspaceIndex, cwd: props.cwd, participants: participants.value, requestId, settings }));
  } catch (e) { error.value = String(e); }
  finally { busy.value = false; }
}
</script>
<template>
  <Teleport to="body">
    <dialog ref="dialog" class="loop-create" aria-labelledby="loop-create-title" @cancel.prevent="close" @keydown.escape.stop>
      <form @submit.prevent="create">
        <header><div><p class="eyebrow">AGENT LOOP</p><h2 id="loop-create-title">Agent Loop 만들기</h2><p class="muted">활성 목록의 첫 프로필로 Agent를 자동 실행합니다.<br />사용량 확인과 계정 전환은 Loop가 관리합니다.</p></div><button type="button" aria-label="닫기" :disabled="busy" @click="close"><Icon icon="lucide:x" /></button></header>
        <div class="body">
          <p class="directory"><Icon icon="lucide:folder" />{{ cwd }}</p>
          <label>Loop 이름<input v-model="name" maxlength="256" required :disabled="busy" autofocus /></label>
          <fieldset :disabled="busy"><legend>참여 계정</legend><label v-for="candidate in loops.state.settings.candidates.filter(c => c.enabled)" :key="key(candidate)" class="participant"><input v-model="participants" type="checkbox" :value="key(candidate)" /><span>{{ candidate.agent === 'codex' ? 'Codex' : 'Claude Code' }} · {{ candidate.label }}</span></label><p v-if="!loops.state.settings.candidates.some(c => c.enabled)" class="muted">설정 → 에이전트 루프에서 사용할 프로필을 활성화하세요.</p></fieldset>
          <div class="options"><label>사용량 임계값 (%)<input v-model.number="threshold" type="number" min="1" max="100" required :disabled="busy" /></label><label>선택 전략<select v-model="strategy" :disabled="busy"><option>SMART</option><option>LEAST_USAGE</option><option>ROUND_ROBIN</option><option>PRIORITY</option></select></label></div>
          <details><summary>고급 설정</summary><div class="options"><label>Usage 조회 간격<select v-model.number="polling"><option :value="10">10초</option><option :value="30">30초</option><option :value="60">60초</option></select></label><label>Interrupt timeout (초)<input v-model.number="interrupt" type="number" min="1" max="10" /></label></div><label class="participant"><input v-model="autoResume" type="checkbox" />Usage reset 후 자동 재개</label></details>
          <p v-if="error" class="error" role="alert">{{ error }}</p>
        </div>
        <footer><span class="muted">선택한 계정을 설정 순서대로 확인한 뒤 실행합니다.</span><button type="button" :disabled="busy" @click="close">취소</button><button class="primary" type="submit" :disabled="busy || !valid">{{ busy ? '생성 중…' : '만들기' }}</button></footer>
      </form>
    </dialog>
  </Teleport>
</template>
<style scoped>
.loop-create{width:min(570px,calc(100vw - 32px));max-height:calc(100dvh - 48px);padding:0;border:1px solid var(--border,#41464d);border-radius:10px;background:var(--bg-primary,#202225);color:var(--text-primary,#e6e8eb);box-shadow:0 24px 80px #0009;font-size:13px}.loop-create::backdrop{background:#0009}form{display:flex;flex-direction:column;max-height:calc(100dvh - 50px)}header,footer{display:flex;align-items:flex-start;justify-content:space-between;gap:12px;padding:20px 24px}header{border-bottom:1px solid #ffffff20}h2{font-size:20px;margin:6px 0 10px}.eyebrow{font-size:11px;letter-spacing:.1em;color:var(--accent);margin:0}.muted,.directory{color:var(--text-secondary,#aeb3ba);line-height:1.6;margin:0}.body{padding:20px 24px;overflow:auto;display:grid;gap:20px;min-height:0}.directory{display:flex;align-items:center;gap:8px;overflow-wrap:anywhere}label{display:flex;flex-direction:column;gap:8px}fieldset{border:1px solid #ffffff20;border-radius:6px;padding:12px;display:grid;gap:12px;max-height:230px;overflow:auto}legend{padding:0 6px}.participant{flex-direction:row;align-items:center;gap:10px}.participant input{accent-color:var(--accent)}input:not([type=checkbox]),select{font:inherit;padding:9px;background:#0002;color:inherit;border:1px solid #ffffff30;border-radius:5px;min-width:0}.options{display:grid;grid-template-columns:1fr 1fr;gap:16px}summary{cursor:pointer;color:var(--text-secondary,#aeb3ba)}details .options{margin:14px 0}button{font:inherit;padding:8px 12px;border:1px solid #ffffff30;border-radius:5px;color:inherit;background:#ffffff08;cursor:pointer}button:disabled{opacity:.45;cursor:default}.primary{background:var(--accent);color:var(--accent-on);border-color:var(--accent)}footer{align-items:center;border-top:1px solid #ffffff20}footer span{flex:1;font-size:12px}.error{color:#ffb3ad;overflow-wrap:anywhere}input:focus-visible,select:focus-visible,button:focus-visible,summary:focus-visible{outline:2px solid var(--accent);outline-offset:2px}@media(max-width:480px){header,.body,footer{padding:16px}.options{grid-template-columns:1fr}footer{flex-wrap:wrap}footer span{flex-basis:100%}}
</style>
