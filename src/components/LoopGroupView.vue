<script setup lang="ts">
import { computed, nextTick, onUnmounted, reactive, ref, watch } from 'vue';
import { Icon } from '@iconify/vue';
import TerminalView from './Terminal.vue';
import { useLoopRouting } from '../composables/useLoopRouting';
import { LOOP_THRESHOLD_BASES, loopBasisLabel, loopStatusLabel, loopWindowLabel, type LoopGroup, type LoopPolicyPatch, type LoopThresholdBasis } from '../lib/loop-routing';
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
/** Loop-wide settings still save on change; per-profile settings use 적용. */
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
      if (patch.strategy != null) input.value = policy.strategy ?? 'SMART';
      else if (patch.pollingIntervalSeconds != null) input.value = String(policy.pollingIntervalSeconds ?? 120);
      else if (input instanceof HTMLInputElement) input.checked = policy.autoResume ?? true;
    }
    busy.value = false;
  }
}
/**
 * Per-profile settings are edited as a local draft and only sent when the
 * profile's own 적용 button is pressed. Saving on `change` (blur) meant a
 * rejected edit — a locked active profile, a value the daemon refused —
 * snapped the field back with the reason buried in the single truncated
 * status line, which read as the change simply not taking effect.
 */
interface ProfileDraft { shortThreshold: number; weeklyThreshold: number; thresholdBasis: LoopThresholdBasis; priority: number }
const DRAFT_FIELDS = ['shortThreshold', 'weeklyThreshold', 'thresholdBasis', 'priority'] as const;
const drafts = reactive<Record<string, ProfileDraft>>({});
const draftErrors = reactive<Record<string, string>>({});
const appliedKey = ref('');
let appliedTimer: ReturnType<typeof setTimeout> | undefined;
onUnmounted(() => clearTimeout(appliedTimer));

function savedDraft(key: string): ProfileDraft | undefined {
  const policy = props.group.policy;
  const candidate = candidatePolicy(key);
  if (!policy || !candidate) return undefined;
  return {
    shortThreshold: candidate.shortThreshold ?? policy.shortThreshold,
    weeklyThreshold: candidate.weeklyThreshold ?? policy.weeklyThreshold,
    thresholdBasis: candidate.thresholdBasis ?? 'short',
    priority: candidate.priority ?? 0,
  };
}
const dirty = (key: string) => {
  const draft = drafts[key];
  const saved = savedDraft(key);
  return !!draft && !!saved && DRAFT_FIELDS.some(field => draft[field] !== saved[field]);
};
// Adopt the daemon's values for every profile that has no unapplied edit, so a
// change made elsewhere shows up without ever discarding what is being typed.
watch(() => [props.group.policy?.shortThreshold, props.group.policy?.weeklyThreshold,
  JSON.stringify(props.group.policy?.candidates ?? []), (props.group.profiles ?? []).map(p => p.key).join('|')],
() => {
  const keys = (props.group.profiles ?? []).map(p => p.key);
  for (const key of keys) {
    const saved = savedDraft(key);
    if (saved && (!drafts[key] || !dirty(key))) drafts[key] = saved;
  }
  for (const key of Object.keys(drafts)) if (!keys.includes(key)) delete drafts[key];
}, { immediate: true, deep: true });

async function applyProfile(key: string, event: Event) {
  const settings = (event.currentTarget as HTMLElement).closest('.profile-settings');
  const invalid = settings?.querySelector<HTMLInputElement>('input:invalid');
  if (invalid) { invalid.reportValidity(); return; }
  const draft = drafts[key];
  const saved = savedDraft(key);
  if (!draft || !saved) return;
  // Only the changed fields travel. That also keeps a locked active profile
  // editable for its priority without tripping the daemon's threshold guard.
  const profile: NonNullable<LoopPolicyPatch['profile']> = { key };
  if (draft.shortThreshold !== saved.shortThreshold) profile.shortThreshold = draft.shortThreshold;
  if (draft.weeklyThreshold !== saved.weeklyThreshold) profile.weeklyThreshold = draft.weeklyThreshold;
  if (draft.thresholdBasis !== saved.thresholdBasis) profile.thresholdBasis = draft.thresholdBasis;
  if (draft.priority !== saved.priority) profile.priority = draft.priority;
  busy.value = true; draftErrors[key] = '';
  try {
    await loops.updatePolicy(props.group.id, { profile });
    await nextTick();
    const applied = savedDraft(key);
    if (applied) drafts[key] = applied;
    appliedKey.value = key;
    clearTimeout(appliedTimer);
    appliedTimer = setTimeout(() => { appliedKey.value = ''; }, 4000);
  } catch (e) {
    draftErrors[key] = String(e);
  } finally {
    busy.value = false;
  }
}
function revertProfile(key: string) {
  const saved = savedDraft(key);
  if (saved) drafts[key] = saved;
  draftErrors[key] = '';
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
      <span v-if="group.switchPending" class="switch-pending" role="status" title="사용량 임계값 초과 — 현재 작업이 끝나면 안전하게 전환합니다"><Icon icon="lucide:clock-arrow-up" />전환 대기</span>
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
        <div class="profile-meta"><span class="window-usage"><span class="window-tag" :title="`${loopWindowLabel(p.thresholdKind)} 사용량`"><Icon :icon="p.thresholdKind === 'short' ? 'lucide:clock' : 'lucide:calendar-range'" />{{ loopWindowLabel(p.thresholdKind) }}</span>{{ percent(p.usage) }} · 임계값 {{ p.threshold }}%</span><span>잔여 {{ percent(p.remaining) }}</span><span v-if="p.resetAt">Reset {{ date(p.resetAt) }}</span></div>
        <div v-if="group.policy && candidatePolicy(p.key) && drafts[p.key]" class="profile-settings">
          <div class="basis-field">
            <span class="basis-label">사용량 기준</span>
            <div class="basis" role="group" :aria-label="p.label + ' 사용량 기준'">
              <button v-for="basis in LOOP_THRESHOLD_BASES" :key="basis.value" type="button" :class="{ on: drafts[p.key].thresholdBasis === basis.value }" :aria-pressed="drafts[p.key].thresholdBasis === basis.value" :title="basis.hint" :disabled="busy || p.key === group.activeProfile" @click="drafts[p.key].thresholdBasis = basis.value"><Icon :icon="basis.value === 'weekly' ? 'lucide:calendar-range' : 'lucide:clock'" />{{ basis.label }}</button>
            </div>
          </div>
          <label :class="{ inactive: drafts[p.key].thresholdBasis !== 'short' }">단기 임계값 (%)<input v-model.number="drafts[p.key].shortThreshold" type="number" min="1" max="100" required :aria-label="p.label + ' 단기 임계값'" :disabled="busy || p.key === group.activeProfile" /></label>
          <label :class="{ inactive: drafts[p.key].thresholdBasis !== 'weekly' }">주간 임계값 (%)<input v-model.number="drafts[p.key].weeklyThreshold" type="number" min="1" max="100" required :aria-label="p.label + ' 주간 임계값'" :disabled="busy || p.key === group.activeProfile" /></label>
          <label>우선순위<input v-model.number="drafts[p.key].priority" type="number" min="-2147483648" max="2147483647" required :aria-label="p.label + ' 우선순위'" :disabled="busy" /></label>
          <span class="basis-hint">{{ loopBasisLabel(drafts[p.key].thresholdBasis) }} 사용량이 임계값에 닿으면 전환합니다. 다른 창은 100% 소진에서만 막습니다.</span>
          <span v-if="p.key === group.activeProfile" class="locked"><Icon icon="lucide:lock-keyhole" />활성 계정의 사용량 기준과 임계값은 변경할 수 없습니다. 우선순위는 바꿀 수 있습니다.</span>
          <div class="apply-row">
            <button type="button" :disabled="busy || !dirty(p.key)" :aria-label="p.label + ' 설정 적용'" @click="applyProfile(p.key, $event)">적용</button>
            <button v-if="dirty(p.key)" type="button" class="ghost" :aria-label="p.label + ' 설정 되돌리기'" @click="revertProfile(p.key)">되돌리기</button>
            <span v-if="dirty(p.key)" class="draft-note" role="status">적용하지 않은 변경이 있습니다</span>
            <span v-else-if="appliedKey === p.key" class="applied-note" role="status"><Icon icon="lucide:check" />적용했습니다</span>
          </div>
          <p v-if="draftErrors[p.key]" class="profile-error" role="alert">{{ draftErrors[p.key] }}</p>
        </div>
        <p v-if="p.usagePending" class="usage-pending">Claude 실행 후 사용량 확인 · 실제 한도 오류 감지 시 자동 전환</p>
        <p v-if="p.error" class="profile-error">{{ p.error }}</p>
      </article></div>
      <fieldset v-if="group.policy" class="session-policy" :disabled="busy"><legend>이 Loop 설정 · 변경 즉시 저장</legend>
        <label>선택 전략<select :value="group.policy.strategy" @change="updatePolicy({ strategy: ($event.target as HTMLSelectElement).value as LoopPolicyPatch['strategy'] }, $event)"><option>SMART</option><option>LEAST_USAGE</option><option>ROUND_ROBIN</option><option>PRIORITY</option></select></label>
        <label>기본 조회 간격 (초)<input type="number" min="60" max="300" required :value="group.policy.pollingIntervalSeconds" @change="updatePolicy({ pollingIntervalSeconds: ($event.target as HTMLInputElement).valueAsNumber }, $event)" /><small>사용량이 높아지면 자동으로 더 자주, 대기 중인 Profile은 덜 자주 확인합니다. Provider 조회 제한 때문에 60초보다 짧게는 확인하지 않습니다</small></label>
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
.loop-view{display:flex;flex-direction:column;height:100%;min-height:0;overflow:hidden}.loop-view :deep(.term-host){flex:1;min-height:0;height:auto}.loop-toolbar{display:flex;align-items:center;gap:10px;flex-wrap:wrap;padding:7px 10px;background:var(--bg-secondary,#252525);font-size:12px;border-bottom:1px solid #ffffff15}.status{display:flex;align-items:center;gap:6px;white-space:nowrap}.dot{width:7px;height:7px;border:1px solid #999;border-radius:50%}.dot.running{background:var(--accent);border-color:var(--accent)}.switch-pending{display:flex;align-items:center;gap:4px;white-space:nowrap;font-size:11px;color:#eac47e;border:1px solid #eac47e50;border-radius:4px;padding:2px 6px}.active-profile{color:var(--accent);max-width:240px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.usage{font-variant-numeric:tabular-nums}.spacer{flex:1}.mini-meter{height:4px;width:80px;border-radius:3px;background:#ffffff20;overflow:hidden}.mini-meter span,.meter>span{display:block;height:100%;background:var(--accent);max-width:100%}button{color:inherit;background:#ffffff08;border:1px solid #ffffff30;border-radius:4px;padding:4px 8px;font:inherit;cursor:pointer}button:disabled{opacity:.4;cursor:default}.expand{display:flex;align-items:center;padding:5px}.latest{padding:4px 10px;margin:0;color:var(--text-secondary,#aeb3ba);font-size:11px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.reset{display:flex;align-items:center;gap:8px;font-size:12px;padding:9px 10px;background:#d9a44115;color:#eac47e}.reset time{margin-left:auto;font-variant-numeric:tabular-nums}.details{max-height:42%;overflow:auto;flex-shrink:0;padding:12px;display:grid;gap:14px;background:var(--bg-secondary,#242424);border-bottom:1px solid #ffffff20;font-size:12px}.profiles{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:10px}.profile{padding:12px;border:1px solid #ffffff20;border-radius:6px}.profile.selected{border-color:var(--accent)}header{display:flex;justify-content:space-between;gap:10px}header span{font-size:10px;color:var(--text-secondary,#aeb3ba)}.meter{height:7px;background:#ffffff18;margin:12px 0 9px;position:relative;border-radius:3px}.meter>span{border-radius:3px}.meter i{position:absolute;width:2px;top:-3px;height:13px;background:#eac47e}.profile-meta{display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;color:var(--text-secondary,#aeb3ba);font-size:11px}.profile-error{color:#ffb3ad;overflow-wrap:anywhere;margin-bottom:0}dl{display:grid;grid-template-columns:auto 1fr;gap:8px;margin:0}dt{color:var(--text-secondary,#aeb3ba)}dd{margin:0;overflow-wrap:anywhere}summary{cursor:pointer;color:var(--text-secondary,#aeb3ba)}ol{list-style:none;padding:0;margin:10px 0;display:grid;gap:7px}li{display:flex;gap:12px}li time{color:var(--text-secondary,#aeb3ba);flex-shrink:0}.advanced button{margin-top:10px}button:focus-visible,summary:focus-visible{outline:2px solid var(--accent);outline-offset:2px}@media(max-width:520px){.mini-meter{display:none}.active-profile{max-width:145px}.loop-toolbar{gap:6px}.profiles{grid-template-columns:1fr}.reset{flex-wrap:wrap}}

.profile-settings { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin-top: 12px; }
.profile-settings label, .session-policy label { display: flex; flex-direction: column; gap: 6px; color: var(--text-secondary, #aeb3ba); font-size: 11px; }
.profile-settings input, .session-policy :is(input[type=number], select) { box-sizing: border-box; min-width: 0; width: 100%; padding: 6px; border: 1px solid #ffffff30; border-radius: 4px; background: var(--bg-primary, #202225); color: var(--text-primary, #e6e8eb); font: inherit; }
.profile-settings input:disabled { opacity: .45; cursor: not-allowed; }
/*
 * The window that is not this account's basis keeps its saved value — flipping
 * the basis back has to bring the number the user had — so it only dims.
 */
.profile-settings label.inactive { opacity: .5; }
.basis-field { grid-column: 1 / -1; display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.basis-label { color: var(--text-secondary, #aeb3ba); font-size: 11px; }
.basis { display: flex; border: 1px solid #ffffff30; border-radius: 5px; overflow: hidden; }
.basis button { display: flex; align-items: center; gap: 5px; padding: 5px 11px; border: 0; border-radius: 0; background: transparent; color: var(--text-secondary, #aeb3ba); font-size: 11px; }
.basis button + button { border-left: 1px solid #ffffff20; }
.basis button.on { background: var(--accent-softer, rgba(90, 155, 255, .22)); color: var(--accent-strong, #82b4ff); font-weight: 600; }
.basis button:disabled { opacity: .45; cursor: not-allowed; }
.basis-hint { grid-column: 1 / -1; color: var(--text-secondary, #aeb3ba); font-size: 10px; line-height: 1.5; }
.window-usage { display: flex; align-items: center; gap: 6px; }
.window-tag { display: inline-flex; align-items: center; gap: 3px; padding: 1px 6px; border: 1px solid var(--accent-border, rgba(90, 155, 255, .32)); border-radius: 999px; background: var(--accent-soft, rgba(90, 155, 255, .12)); color: var(--accent-strong, #82b4ff); font-size: 10px; }
.locked { grid-column: 1 / -1; display: flex; align-items: center; gap: 5px; color: var(--text-secondary, #aeb3ba); font-size: 11px; }
.apply-row { grid-column: 1 / -1; display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.apply-row .ghost { background: transparent; border-color: #ffffff20; }
.draft-note { font-size: 11px; color: #eac47e; }
.applied-note { display: flex; align-items: center; gap: 3px; font-size: 11px; color: var(--accent); }
.profile-settings .profile-error { grid-column: 1 / -1; margin: 0; font-size: 11px; }
.session-policy { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; border: 1px solid #ffffff20; border-radius: 6px; padding: 12px; }
.session-policy .auto-resume { flex-direction: row; align-items: center; }
.session-policy small { font-size: 10px; color: var(--text-secondary, #aeb3ba); font-weight: 400; }
.session-policy input[type=checkbox] { accent-color: var(--accent); }
input:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.usage-pending { margin: 10px 0 0; font-size: 11px; color: var(--text-secondary, #aeb3ba); }
</style>
