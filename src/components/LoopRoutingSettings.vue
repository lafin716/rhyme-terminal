<script setup lang="ts">
import { ref, watch } from 'vue';
import { useLoopRouting, saveLoopSettings } from '../composables/useLoopRouting';
import { mergeLoopProfiles, type LoopSettings, type LoopAgent } from '../lib/loop-routing';
import { useAccountProfiles } from '../composables/useAccountProfiles';
const { state } = useLoopRouting();
const clone = () => JSON.parse(JSON.stringify(mergeLoopProfiles(state.settings, useAccountProfiles().profiles))) as LoopSettings;
const draft = ref(clone());
const busy = ref(false);
const message = ref('');
watch(() => useAccountProfiles().profiles, () => { draft.value = mergeLoopProfiles(draft.value, useAccountProfiles().profiles); }, { deep: true });
function moveAgent(agent: LoopAgent) { draft.value.agentOrder = [agent, ...draft.value.agentOrder.filter(a => a !== agent)]; }
function moveCandidate(index: number, delta: number) {
  const candidates = draft.value.candidates;
  const agent = candidates[index].agent;
  let target = index + delta;
  while (target >= 0 && target < candidates.length && candidates[target].agent !== agent) target += delta;
  if (target >= 0 && target < candidates.length) [candidates[index], candidates[target]] = [candidates[target], candidates[index]];
}
async function save() {
  busy.value = true; message.value = '';
  try { await saveLoopSettings(draft.value); message.value = '저장했습니다.'; } catch(e) { message.value = String(e); }
  finally { busy.value = false; }
}
</script>
<template>
  <form class="loop-settings" @submit.prevent="save">
    <header>
    <h2>에이전트 루프</h2>
    <p>세션 메뉴에서 만든 루프 그룹에만 적용됩니다. 사용량이 기준에 도달하면 작업 완료 후 다음 계정으로 전환합니다.</p>
    </header>
    <section class="card">
    <h3>공통 전환 기준</h3>
    <div class="thresholds"><label>단기 사용량 기준 (%) <input v-model.number="draft.shortThreshold" type="number" min="1" max="100" required /></label><label>주간 사용량 기준 (%) <input v-model.number="draft.weeklyThreshold" type="number" min="1" max="100" required /></label></div>
    </section>
    <section class="card agent-order">
    <p>에이전트 순서: {{ draft.agentOrder.join(' → ') }}</p>
    <button type="button" @click="moveAgent(draft.agentOrder[1])">에이전트 순서 바꾸기</button>
    </section>
    <section v-for="agent in draft.agentOrder" :key="agent" class="card">
      <h3>{{ agent === 'claude' ? 'Claude Code' : 'Codex' }}</h3>
      <div v-for="{candidate, index} in draft.candidates.map((candidate,index) => ({candidate,index})).filter(row => row.candidate.agent === agent)" :key="candidate.profileId ?? 'system'" class="candidate">
        <label class="account"><input v-model="candidate.enabled" type="checkbox" />{{ candidate.label }}</label>
        <label>단기 <input v-model.number="candidate.shortThreshold" type="number" min="1" max="100" :placeholder="String(draft.shortThreshold)" @change="candidate.shortThreshold = candidate.shortThreshold || null" /></label>
        <label>주간 <input v-model.number="candidate.weeklyThreshold" type="number" min="1" max="100" :placeholder="String(draft.weeklyThreshold)" @change="candidate.weeklyThreshold = candidate.weeklyThreshold || null" /></label>
        <button type="button" :aria-label="candidate.label + ' 우선순위 올리기'" @click="moveCandidate(index,-1)">↑</button><button type="button" :aria-label="candidate.label + ' 우선순위 내리기'" @click="moveCandidate(index,1)">↓</button>
      </div>
    </section>
    <p>체크한 계정만 위 순서대로 사용합니다. 시스템 계정과 새 프로필은 기본적으로 제외되며, 빈 기준값은 공통 설정을 따릅니다.</p>
    <div class="save-actions">
    <button type="submit" :disabled="busy">{{ busy ? '저장 중…' : '저장' }}</button>
    <p role="status">{{ message || state.error }}</p>
    </div>
  </form>
</template>
<style scoped>
.loop-settings { display: flex; flex-direction: column; gap: 24px; min-width: 0; color: #d4d4d4; }
h2 { margin: 0 0 6px; font-size: 20px; font-weight: 600; color: #e6e6e6; }
h3 { margin: 0 0 16px; font-size: 14.5px; font-weight: 600; color: #e6e6e6; }
.loop-settings p { margin: 0; max-width: 80ch; color: #aaaaaa; font-size: 13px; line-height: 1.7; overflow-wrap: anywhere; }
.card { min-width: 0; background: #202020; border: 1px solid #2b2b2b; border-radius: 10px; padding: 24px; }
.thresholds { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 18px 20px; }
.thresholds label { display: flex; flex-direction: column; gap: 8px; }
.thresholds input { width: 100%; }
.agent-order, .candidate, .save-actions { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.agent-order { justify-content: space-between; gap: 16px; }
.candidate { padding: 16px 0; border-top: 1px solid #333333; }
.candidate:last-child { padding-bottom: 0; }
.candidate label { display: flex; align-items: center; gap: 8px; }
.account { min-width: 160px; flex: 1; overflow-wrap: anywhere; color: #e6e6e6; cursor: pointer; }
input[type=number] { box-sizing: border-box; width: 84px; min-width: 0; min-height: 40px; padding: 9px 12px; color: #e6e6e6; background: #252525; border: 1px solid #333333; border-radius: 6px; font: inherit; font-size: 13px; }
.thresholds input { width: 100%; }
input[type=number]:focus { outline: none; border-color: #4ec9b0; }
input[type=checkbox] { width: 16px; height: 16px; margin: 0; flex-shrink: 0; accent-color: #4ec9b0; }
button { display: inline-flex; align-items: center; justify-content: center; min-height: 38px; flex-shrink: 0; background: #202020; color: #d4d4d4; border: 1px solid #333333; padding: 7px 14px; border-radius: 6px; font: inherit; font-size: 13px; cursor: pointer; white-space: nowrap; }
button:hover:not(:disabled) { background: #2a2a2a; }
button:disabled { color: #555555; cursor: not-allowed; }
.loop-settings :is(button, input):focus-visible { outline: 2px solid #4ec9b0; outline-offset: 2px; }
label { font-size: 13px; color: #bbbbbb; }
@media (max-width: 760px) {
  .card { padding: 18px; }
  .thresholds { grid-template-columns: minmax(0, 1fr); }
  .account { flex-basis: 100%; }
}
</style>
