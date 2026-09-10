<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { Icon } from '@iconify/vue';
import TerminalView from './Terminal.vue';
import { useLoopRouting } from '../composables/useLoopRouting';
import { loopStatusLabel, type LoopGroup, type LoopPolicyPatch } from '../lib/loop-routing';
const props = defineProps<{ group: LoopGroup; active: boolean }>();
const loops = useLoopRouting();
const expanded = ref(false);
const busy = ref(false);
const error = ref('');
const now = ref(Date.now());
watch(() => props.group.status, (status, _, cleanup) => {
  if (status !== 'waiting_for_usage_reset') return;
  now.value = Date.now();
  const timer = setInterval(() => { now.value = Date.now(); }, 1000);
  cleanup(() => clearInterval(timer));
}, { immediate: true });
const profile = computed(() => props.group.profiles?.find(p => p.key === props.group.activeProfile));
const waitingProfile = computed(() => props.group.profiles?.find(p => p.key === props.group.waitingProfileId));
const latest = computed(() => props.group.events?.slice(-1)[0]);
const uncertainInput = computed(() => props.group.queuedInput?.filter(c => c.delivering) ?? []);
const inputText = computed(() => new TextDecoder().decode(new Uint8Array(uncertainInput.value.flatMap(c => c.bytes))));
async function resolveInput(delivered: boolean) {
  busy.value = true; error.value = '';
  try { await loops.resolveInput(props.group.id, delivered); } catch (e) { error.value = String(e); }
  finally { busy.value = false; }
}
const countdown = computed(() => {
  const seconds = Math.max(0, Math.ceil(((props.group.resumeAt ?? now.value) - now.value) / 1000));
  return [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60].map(n => String(n).padStart(2, '0')).join(':');
});
const date = (value: number) => new Date(value).toLocaleTimeString('ko-KR', { hour12: false });
const candidatePolicy = (key: string) => props.group.policy?.candidates.find(c => c.agent + ':' + (c.profileId ?? 'system') === key);
async function updatePolicy(patch: LoopPolicyPatch, event: Event) {
  const input = event.target as HTMLInputElement | HTMLSelectElement;
  if (!input.checkValidity()) { input.reportValidity(); return; }
  busy.value = true; error.value = '';
  try { await loops.updatePolicy(props.group.id, patch); }
  catch (e) { error.value = String(e); }
  finally {
    await nextTick();
    // Restore the authoritative value after success, validation races or a rejected request.
    const policy = props.group.policy;
    if (policy) {
      if (patch.profile) {
        const candidate = candidatePolicy(patch.profile.key);
        if (patch.profile.shortThreshold != null) input.value = String(candidate?.shortThreshold ?? policy.shortThreshold);
        else if (patch.profile.weeklyThreshold != null) input.value = String(candidate?.weeklyThreshold ?? policy.weeklyThreshold);
        else input.value = String(candidate?.priority ?? 0);
      } else if (patch.strategy != null) input.value = policy.strategy ?? 'SMART';
      else if (patch.pollingIntervalSeconds != null) input.value = String(policy.pollingIntervalSeconds ?? 30);
      else if (input instanceof HTMLInputElement) input.checked = policy.autoResume ?? true;
    }
    busy.value = false;
  }
}
function updateProfile(key: string, field: 'shortThreshold' | 'weeklyThreshold' | 'priority', event: Event) {
  const input = event.target as HTMLInputElement;
  void updatePolicy({ profile: { key, [field]: input.valueAsNumber } }, event);
}
const percent = (value: number | null | undefined) => value == null ? '확인 전' : `${Math.round(value)}%`;
async function control(op: 'pause' | 'resume' | 'next' | 'stop') {
  busy.value = true; error.value = '';
  try { await loops.control(op, props.group.id); } catch (e) { error.value = String(e); }
  finally { busy.value = false; }
}
</script>
<template>
  <section class="loop-view">
    <div v-if="uncertainInput.length" class="input-recovery"><p>재시작 전 아래 입력의 전달 여부를 확인할 수 없습니다. Agent 대화에서 확인한 뒤 재개하세요.</p><pre>{{ inputText }}</pre><button :disabled="busy" @click="resolveInput(true)">이미 전달됨</button> <button :disabled="busy" @click="resolveInput(false)">전달 안 됨 — 재개 시 전송</button></div>
    <div class="loop-toolbar">
      <span class="status" role="status"><span class="dot" :class="{ running: group.runtime?.pid != null }" />{{ loopStatusLabel(group.status) }}</span>
      <strong v-if="profile" class="active-profile">{{ profile.agent === 'codex' ? 'Codex' : 'Claude Code' }} · {{ profile.label }}</strong>
      <span v-if="profile" class="usage">{{ percent(profile.usage) }}</span>
      <div v-if="profile" class="mini-meter"><span :style="{ width: `${profile.usage ?? 0}%` }" /></div>
      <span class="spacer" />
      <button :disabled="busy || group.status === 'stopped'" @click="control(['paused', 'error', 'recovery'].includes(group.status) ? 'resume' : 'pause')">{{ ['paused', 'error', 'recovery'].includes(group.status) ? '재개' : '일시정지' }}</button>
      <button :disabled="busy || group.status === 'stopped'" @click="control('stop')">종료</button>
      <button class="expand" :aria-expanded="expanded" aria-label="Agent Loop 상세 정보" @click="expanded = !expanded"><Icon :icon="expanded ? 'lucide:chevron-up' : 'lucide:chevron-down'" /></button>
    </div>
    <div v-if="group.status === 'waiting_for_usage_reset'" class="reset" role="status"><Icon icon="lucide:hourglass" /><span>{{ waitingProfile ? `${waitingProfile.agent} · ${waitingProfile.label}` : '참여 Profile' }} · {{ group.resumeAt ? date(group.resumeAt) : '확인 중' }} 재확인</span><time>{{ countdown }}</time></div>
    <p v-if="error || latest || group.reason" class="latest" :title="error || latest?.message || group.reason || ''">{{ error || (latest ? `${date(latest.at)} ${latest.message}` : group.reason) }}</p>
    <div v-if="expanded" class="details" aria-label="Agent Loop 상세 정보">
      <div class="profiles"><article v-for="p in group.profiles ?? []" :key="p.key" class="profile" :class="{ selected: p.key === group.activeProfile }">
        <header><strong>{{ p.agent === 'codex' ? 'Codex' : 'Claude Code' }} · {{ p.label }}</strong><span>{{ p.status }}</span></header>
        <div class="meter" role="progressbar" :aria-label="`${p.label} 사용량`" :aria-valuenow="p.usage ?? undefined" :aria-valuemin="0" :aria-valuemax="100"><span :style="{ width: `${p.usage ?? 0}%` }" /><i :style="{ left: `${p.threshold}%` }" :title="`임계값 ${p.threshold}%`" /></div>
        <div class="profile-meta"><span>{{ percent(p.usage) }} · 임계값 {{ p.threshold }}%</span><span>잔여 {{ percent(p.remaining) }}</span><span v-if="p.resetAt">Reset {{ date(p.resetAt) }}</span></div>
        <div v-if="group.policy && candidatePolicy(p.key)" class="profile-settings">
          <label>단기 임계값 (%)<input type="number" min="1" max="100" required :aria-label="p.label + ' 단기 임계값'" :value="candidatePolicy(p.key)?.shortThreshold ?? group.policy.shortThreshold" :disabled="busy || p.key === group.activeProfile" @change="updateProfile(p.key, 'shortThreshold', $event)" /></label>
          <label>주간 임계값 (%)<input type="number" min="1" max="100" required :aria-label="p.label + ' 주간 임계값'" :value="candidatePolicy(p.key)?.weeklyThreshold ?? group.policy.weeklyThreshold" :disabled="busy || p.key === group.activeProfile" @change="updateProfile(p.key, 'weeklyThreshold', $event)" /></label>
          <label>우선순위<input type="number" min="-2147483648" max="2147483647" required :aria-label="p.label + ' 우선순위'" :value="candidatePolicy(p.key)?.priority ?? 0" :disabled="busy" @change="updateProfile(p.key, 'priority', $event)" /></label>
          <span v-if="p.key === group.activeProfile" class="locked"><Icon icon="lucide:lock-keyhole" />활성 계정의 임계값은 변경할 수 없습니다.</span>
        </div>
        <p v-if="p.usagePending" class="usage-pending">Claude 실행 후 사용량 확인 · 실제 한도 오류 감지 시 자동 전환</p>
        <p v-if="p.error" class="profile-error">{{ p.error }}</p>
      </article></div>
      <fieldset v-if="group.policy" class="session-policy" :disabled="busy"><legend>이 Loop 설정 · 변경 즉시 저장</legend>
        <label>선택 전략<select :value="group.policy.strategy" @change="updatePolicy({ strategy: ($event.target as HTMLSelectElement).value as LoopPolicyPatch['strategy'] }, $event)"><option>SMART</option><option>LEAST_USAGE</option><option>ROUND_ROBIN</option><option>PRIORITY</option></select></label>
        <label>조회 간격 (초)<input type="number" min="10" max="300" required :value="group.policy.pollingIntervalSeconds" @change="updatePolicy({ pollingIntervalSeconds: ($event.target as HTMLInputElement).valueAsNumber }, $event)" /></label>
        <label class="auto-resume"><input type="checkbox" :checked="group.policy.autoResume" @change="updatePolicy({ autoResume: ($event.target as HTMLInputElement).checked }, $event)" />Usage reset 후 자동 재개</label>
      </fieldset>
      <dl><dt>현재 Agent</dt><dd>{{ group.currentProvider ?? '없음' }} · {{ group.runtime?.status ?? 'IDLE' }}</dd><dt>Session</dt><dd>{{ group.currentAgentSessionId ?? 'Agent 실행 후 확인' }}</dd><dt>작업 폴더</dt><dd>{{ group.cwd }}</dd></dl>
      <details class="timeline"><summary>System Event 기록 ({{ group.events?.length ?? 0 }})</summary><ol><li v-for="(event, index) in group.events ?? []" :key="index"><time>{{ date(event.at) }}</time><span>{{ event.message }}</span></li></ol></details>
      <details class="advanced"><summary>고급 제어</summary><button :disabled="busy || !['running', 'preparing'].includes(group.status)" @click="control('next')">지금 Profile 전환</button></details>
    </div>
    <TerminalView v-if="group.activeSessionId" :key="group.activeSessionId" :session-id="group.activeSessionId" :active="active" />
    <p v-else class="latest">{{ group.status === 'stopped' ? '종료된 Loop입니다.' : '터미널을 연결하는 중입니다.' }}</p>
  </section>
</template>
<style scoped>
.input-recovery{padding:10px;font-size:12px;background:#d9a44115}.input-recovery pre{max-height:100px;overflow:auto;white-space:pre-wrap}
.loop-view{display:flex;flex-direction:column;height:100%;min-height:0;overflow:hidden}.loop-view :deep(.term-host){flex:1;min-height:0;height:auto}.loop-toolbar{display:flex;align-items:center;gap:10px;flex-wrap:wrap;padding:7px 10px;background:var(--bg-secondary,#252525);font-size:12px;border-bottom:1px solid #ffffff15}.status{display:flex;align-items:center;gap:6px;white-space:nowrap}.dot{width:7px;height:7px;border:1px solid #999;border-radius:50%}.dot.running{background:var(--accent);border-color:var(--accent)}.active-profile{color:var(--accent);max-width:240px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.usage{font-variant-numeric:tabular-nums}.spacer{flex:1}.mini-meter{height:4px;width:80px;border-radius:3px;background:#ffffff20;overflow:hidden}.mini-meter span,.meter>span{display:block;height:100%;background:var(--accent);max-width:100%}button{color:inherit;background:#ffffff08;border:1px solid #ffffff30;border-radius:4px;padding:4px 8px;font:inherit;cursor:pointer}button:disabled{opacity:.4;cursor:default}.expand{display:flex;align-items:center;padding:5px}.latest{padding:4px 10px;margin:0;color:var(--text-secondary,#aeb3ba);font-size:11px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.reset{display:flex;align-items:center;gap:8px;font-size:12px;padding:9px 10px;background:#d9a44115;color:#eac47e}.reset time{margin-left:auto;font-variant-numeric:tabular-nums}.details{max-height:42%;overflow:auto;flex-shrink:0;padding:12px;display:grid;gap:14px;background:var(--bg-secondary,#242424);border-bottom:1px solid #ffffff20;font-size:12px}.profiles{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:10px}.profile{padding:12px;border:1px solid #ffffff20;border-radius:6px}.profile.selected{border-color:var(--accent)}header{display:flex;justify-content:space-between;gap:10px}header span{font-size:10px;color:var(--text-secondary,#aeb3ba)}.meter{height:7px;background:#ffffff18;margin:12px 0 9px;position:relative;border-radius:3px}.meter>span{border-radius:3px}.meter i{position:absolute;width:2px;top:-3px;height:13px;background:#eac47e}.profile-meta{display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;color:var(--text-secondary,#aeb3ba);font-size:11px}.profile-error{color:#ffb3ad;overflow-wrap:anywhere;margin-bottom:0}dl{display:grid;grid-template-columns:auto 1fr;gap:8px;margin:0}dt{color:var(--text-secondary,#aeb3ba)}dd{margin:0;overflow-wrap:anywhere}summary{cursor:pointer;color:var(--text-secondary,#aeb3ba)}ol{list-style:none;padding:0;margin:10px 0;display:grid;gap:7px}li{display:flex;gap:12px}li time{color:var(--text-secondary,#aeb3ba);flex-shrink:0}.advanced button{margin-top:10px}button:focus-visible,summary:focus-visible{outline:2px solid var(--accent);outline-offset:2px}@media(max-width:520px){.mini-meter{display:none}.active-profile{max-width:145px}.loop-toolbar{gap:6px}.profiles{grid-template-columns:1fr}.reset{flex-wrap:wrap}}

.profile-settings { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin-top: 12px; }
.profile-settings label, .session-policy label { display: flex; flex-direction: column; gap: 6px; color: var(--text-secondary, #aeb3ba); font-size: 11px; }
.profile-settings input, .session-policy :is(input[type=number], select) { box-sizing: border-box; min-width: 0; width: 100%; padding: 6px; border: 1px solid #ffffff30; border-radius: 4px; background: var(--bg-primary, #202225); color: var(--text-primary, #e6e8eb); font: inherit; }
.profile-settings input:disabled { opacity: .45; cursor: not-allowed; }
.locked { grid-column: 1 / -1; display: flex; align-items: center; gap: 5px; color: var(--text-secondary, #aeb3ba); font-size: 11px; }
.session-policy { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; border: 1px solid #ffffff20; border-radius: 6px; padding: 12px; }
.session-policy .auto-resume { flex-direction: row; align-items: center; }
.session-policy input[type=checkbox] { accent-color: var(--accent); }
input:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.usage-pending { margin: 10px 0 0; font-size: 11px; color: var(--text-secondary, #aeb3ba); }
</style>
