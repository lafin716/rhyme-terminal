<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { Icon } from '@iconify/vue';
import { useLoopRouting, saveLoopSettings } from '../composables/useLoopRouting';
import { LOOP_THRESHOLD_BASES, mergeLoopProfiles, type LoopAgent, type LoopSettings, type LoopCandidate } from '../lib/loop-routing';
import { useAccountProfiles } from '../composables/useAccountProfiles';

const { state } = useLoopRouting();
const clone = () =>
  JSON.parse(JSON.stringify(mergeLoopProfiles(state.settings, useAccountProfiles().profiles))) as LoopSettings;
const draft = ref(clone());
const busy = ref(false);
const message = ref('');

watch(
  () => useAccountProfiles().profiles,
  () => {
    endDrag();
    draft.value = mergeLoopProfiles(draft.value, useAccountProfiles().profiles);
  },
  { deep: true },
);
const zones = [
  { enabled: true, title: '활성 프로필' },
  { enabled: false, title: '대기 프로필' },
];
const active = computed(() => draft.value.candidates.filter(c => c.enabled));
const dragged = ref<LoopCandidate | null>(null);
const key = (c: LoopCandidate) => c.agent + ':' + (c.profileId ?? 'system');
const dropTarget = ref<{ enabled: boolean; before?: string } | null>(null);
let pointer:
  | { id: number; handle: HTMLElement; candidate: LoopCandidate; startX: number; startY: number; x: number; y: number }
  | null = null;
let scrollFrame = 0;
/**
 * Claude's `--effort` levels, verbatim from its own help. Codex has no
 * `--effort` flag — its reasoning effort lives in `~/.codex/config.toml` — so
 * the control is not offered for it, and `prepare_launch` no longer sends one.
 */
const effortOptions: Record<LoopAgent, string[]> = {
  claude: ['low', 'medium', 'high', 'xhigh', 'max'],
  codex: [],
};
/**
 * Claude's `--permission-mode` choices, verbatim from its own help. Codex has
 * no equivalent flag (it uses `--ask-for-approval` / `--sandbox`), so nothing
 * is offered there either.
 */
const modeOptions: Record<LoopAgent, string[]> = {
  claude: ['acceptEdits', 'auto', 'bypassPermissions', 'manual', 'dontAsk', 'plan'],
  codex: [],
};
/**
 * Known `--model` values per agent, offered as a picker because a typo here
 * only surfaces later, when the Loop launches the CLI and it exits.
 *
 * Claude's values are the ones its own `--model` help documents: an alias for
 * the latest model in a tier, or a model's full name. Aliases are listed first
 * since they keep following that tier as new models ship.
 *
 * Codex has no list: its `--model` is documented only as "Model the agent
 * should use", with no enumerated values, so anything here would be a guess.
 * Its picker therefore offers 기본, whatever is already saved, and 직접 입력.
 */
const modelOptions: Record<LoopAgent, string[]> = {
  claude: ['fable', 'opus', 'sonnet', 'haiku', 'claude-fable-5-1', 'claude-opus-5', 'claude-sonnet-5', 'claude-haiku-4-5'],
  codex: [],
};
/** Keeps a value the list does not know — an older pin, or a Codex model — selectable. */
const modelChoices = (candidate: LoopCandidate) => {
  const known = modelOptions[candidate.agent] ?? [];
  return candidate.model && !known.includes(candidate.model) ? [...known, candidate.model] : known;
};
// Profiles switched to free text, so a value the picker cannot offer stays reachable.
const customModel = ref<Record<string, boolean>>({});
/** Sentinel option value. Not a possible model id, so it cannot collide. */
const MODEL_CUSTOM = '__custom__';
function onModelSelect(candidate: LoopCandidate, event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  if (value === MODEL_CUSTOM) {
    customModel.value = { ...customModel.value, [key(candidate)]: true };
    return;
  }
  candidate.model = normalizeOptional(value);
}

function normalizeOptional(value: string | null | undefined) {
  const normalized = value?.trim();
  return normalized ? normalized : null;
}

function findDropTarget() {
  if (!pointer || !dragged.value) return;
  const zone = document
    .elementFromPoint(pointer.x, pointer.y)
    ?.closest<HTMLElement>('.profile-zone');
  if (!zone || !pointer.handle.closest('.profile-board')?.contains(zone)) {
    dropTarget.value = null;
    return;
  }
  const before = [...zone.querySelectorAll<HTMLElement>('.candidate')].find(row => {
    const bounds = row.getBoundingClientRect();
    return (
      row.dataset.profileKey !== key(dragged.value!) &&
      pointer!.y < bounds.top + bounds.height / 2
    );
  });
  dropTarget.value = { enabled: zone.dataset.enabled === 'true', before: before?.dataset.profileKey };
}
function scrollWhileDragging() {
  if (!pointer || !dragged.value) return;
  const scroller = pointer.handle.closest<HTMLElement>('.content');
  if (scroller) {
    const bounds = scroller.getBoundingClientRect();
    const delta = pointer.y < bounds.top + 40 ? -12 : pointer.y > bounds.bottom - 40 ? 12 : 0;
    if (delta) {
      scroller.scrollTop += delta;
      findDropTarget();
    }
  }
  scrollFrame = requestAnimationFrame(scrollWhileDragging);
}
function endDrag() {
  const previous = pointer;
  pointer = null;
  cancelAnimationFrame(scrollFrame);
  if (previous?.handle.hasPointerCapture(previous.id)) {
    previous.handle.releasePointerCapture(previous.id);
  }
  dragged.value = null;
  dropTarget.value = null;
  window.removeEventListener('pointermove', pointerMove);
  window.removeEventListener('pointerup', pointerUp);
  window.removeEventListener('pointercancel', endDrag);
  window.removeEventListener('blur', endDrag);
  window.removeEventListener('keydown', cancelOnEscape, true);
}
function cancelOnEscape(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    endDrag();
  }
}
function pointerMove(event: PointerEvent) {
  if (!pointer || event.pointerId !== pointer.id) return;
  pointer.x = event.clientX;
  pointer.y = event.clientY;
  if (
    !dragged.value &&
    Math.hypot(pointer.x - pointer.startX, pointer.y - pointer.startY) >= 4
  ) {
    dragged.value = pointer.candidate;
    scrollFrame = requestAnimationFrame(scrollWhileDragging);
  }
  findDropTarget();
}
function pointerUp(event: PointerEvent) {
  if (!pointer || event.pointerId !== pointer.id) return;
  pointerMove(event);
  const candidate = dragged.value;
  const target = dropTarget.value;
  if (candidate && target) {
    place(candidate, target.enabled, draft.value.candidates.find(c => key(c) === target.before));
  }
  endDrag();
}
function startDrag(event: PointerEvent, candidate: LoopCandidate) {
  if (event.button !== 0 || busy.value) return;
  endDrag();
  const handle = event.currentTarget as HTMLElement;
  handle.focus();
  pointer = {
    id: event.pointerId,
    handle,
    candidate,
    startX: event.clientX,
    startY: event.clientY,
    x: event.clientX,
    y: event.clientY,
  };
  handle.setPointerCapture(event.pointerId);
  window.addEventListener('pointermove', pointerMove);
  window.addEventListener('pointerup', pointerUp);
  window.addEventListener('pointercancel', endDrag);
  window.addEventListener('blur', endDrag);
  window.addEventListener('keydown', cancelOnEscape, true);
}
onBeforeUnmount(endDrag);
function place(candidate: LoopCandidate, enabled: boolean, before?: LoopCandidate) {
  if (candidate === before) return;
  const rows = draft.value.candidates;
  const sourceIndex = rows.indexOf(candidate);
  if (sourceIndex < 0) return;
  rows.splice(sourceIndex, 1);
  candidate.enabled = enabled;
  rows.splice(before ? rows.indexOf(before) : rows.length, 0, candidate);
}
function move(candidate: LoopCandidate, delta: number) {
  const rows = draft.value.candidates.filter(c => c.enabled === candidate.enabled);
  const index = rows.indexOf(candidate);
  const target = index + delta;
  if (target < 0 || target >= rows.length) return;
  if (delta < 0) place(candidate, candidate.enabled, rows[target]);
  else place(rows[target], candidate.enabled, candidate);
}
async function save() {
  busy.value = true;
  message.value = '';
  try {
    await saveLoopSettings(draft.value);
    message.value = '저장했습니다.';
  } catch (e) {
    message.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <form class="loop-settings" @submit.prevent="save">
    <header>
      <h2>에이전트 루프</h2>
      <p>사용할 프로필을 활성 영역으로 옮기세요. 새 Loop는 활성 순서의 첫 프로필부터 사용량을 확인하고 자동 실행합니다.</p>
    </header>
    <section class="card">
      <h3>공통 전환 기준</h3>
      <div class="thresholds">
        <label
          >단기 사용량 기준 (%)
          <input v-model.number="draft.shortThreshold" type="number" min="1" max="100" required />
        </label>
        <label
          >주간 사용량 기준 (%)
          <input v-model.number="draft.weeklyThreshold" type="number" min="1" max="100" required />
        </label>
      </div>
      <p class="basis-note">
        전환에 쓰는 창은 계정마다 고릅니다 — 각 프로필의 <strong>개별 기준 → 사용량 기준</strong>에서 5시간과 주간 중 하나를 선택하세요.
        기본값은 5시간이고, 기준이 아닌 창은 100% 소진에서만 막습니다.
      </p>
      <div class="thresholds">
        <label
          >선택 전략
          <select v-model="draft.strategy">
            <option>SMART</option>
            <option>LEAST_USAGE</option>
            <option>ROUND_ROBIN</option>
            <option>PRIORITY</option>
          </select>
        </label>
        <label
          >기본 Usage 조회 간격 (초)
          <input v-model.number="draft.pollingIntervalSeconds" type="number" min="60" max="300" />
        </label>
      </div>
    </section>
    <section class="card profile-board" aria-label="전체 에이전트 프로필">
      <div
        v-for="zone in zones"
        :key="String(zone.enabled)"
        class="profile-zone"
        :data-enabled="zone.enabled"
        :class="{ 'drop-end': dropTarget?.enabled === zone.enabled && !dropTarget.before }"
      >
        <h3>{{ zone.title }} · {{ draft.candidates.filter(c => c.enabled === zone.enabled).length }}</h3>
        <p>
          {{ zone.enabled ? '숫자는 전체 실행 순서입니다. 핸들을 다른 행 앞으로 끌어 순서를 바꾸세요.' : '대기 프로필은 Loop에 참여하지 않습니다. 활성 영역으로 끌어 추가하세요.' }}
        </p>
        <div class="profile-list">
          <div
            v-for="candidate in draft.candidates.filter(c => c.enabled === zone.enabled)"
            :key="key(candidate)"
            class="candidate"
            :data-profile-key="key(candidate)"
            :class="{ dragging: dragged === candidate, 'drop-before': dropTarget?.before === key(candidate) }"
          >
            <div class="profile-row">
              <button
                type="button"
                class="drag-handle"
                :draggable="false"
                :aria-label="candidate.label + ' 순서 변경 (위아래 방향키)'"
                @pointerdown.prevent.stop="startDrag($event, candidate)"
                @lostpointercapture="endDrag"
                @dragstart.prevent
                @keydown.up.prevent="move(candidate, -1)"
                @keydown.down.prevent="move(candidate, 1)"
              >
                <Icon icon="lucide:grip-vertical" />
              </button>
              <span v-if="zone.enabled" class="order">{{ active.indexOf(candidate) + 1 }}</span>
              <span class="provider-tag" :class="candidate.agent">{{ candidate.agent === 'claude' ? 'Claude Code' : 'Codex' }}</span>
              <span class="profile-name">{{ candidate.label }}</span>
              <button type="button" class="toggle-profile" @click="place(candidate, !zone.enabled)">
                {{ zone.enabled ? '대기로' : '활성화' }}
              </button>
            </div>
            <details>
              <summary>개별 기준</summary>
              <div class="profile-options">
                <div class="basis-field">
                  <span class="basis-label">사용량 기준</span>
                  <div class="basis" role="group" :aria-label="candidate.label + ' 사용량 기준'">
                    <button
                      v-for="basis in LOOP_THRESHOLD_BASES"
                      :key="basis.value"
                      type="button"
                      :class="{ on: (candidate.thresholdBasis ?? 'short') === basis.value }"
                      :aria-pressed="(candidate.thresholdBasis ?? 'short') === basis.value"
                      :title="basis.hint"
                      @click="candidate.thresholdBasis = basis.value"
                    >
                      <Icon :icon="basis.value === 'weekly' ? 'lucide:calendar-range' : 'lucide:clock'" />{{ basis.label }}
                    </button>
                  </div>
                  <small>이 계정을 전환할 창입니다. 나머지 창은 100% 소진에서만 막습니다</small>
                </div>
                <label :class="{ inactive: (candidate.thresholdBasis ?? 'short') !== 'short' }"
                  >단기 (%)
                  <input
                    v-model.number="candidate.shortThreshold"
                    type="number"
                    min="1"
                    max="100"
                    :placeholder="String(draft.shortThreshold)"
                    @change="candidate.shortThreshold = candidate.shortThreshold || null"
                  />
                </label>
                <label :class="{ inactive: (candidate.thresholdBasis ?? 'short') !== 'weekly' }"
                  >주간 (%)
                  <input
                    v-model.number="candidate.weeklyThreshold"
                    type="number"
                    min="1"
                    max="100"
                    :placeholder="String(draft.weeklyThreshold)"
                    @change="candidate.weeklyThreshold = candidate.weeklyThreshold || null"
                  />
                </label>
                <label>우선순위 <input v-model.number="candidate.priority" type="number" /></label>
                <label
                  >모델
                  <select
                    v-if="!customModel[key(candidate)]"
                    :value="candidate.model ?? ''"
                    @change="onModelSelect(candidate, $event)"
                  >
                    <option value="">{{ candidate.agent }} 기본</option>
                    <option v-for="model in modelChoices(candidate)" :key="model" :value="model">{{ model }}</option>
                    <option :value="MODEL_CUSTOM">직접 입력…</option>
                  </select>
                  <span v-else class="model-custom">
                    <input
                      v-model="candidate.model"
                      type="text"
                      :placeholder="`${candidate.agent} 기본`"
                      @change="candidate.model = normalizeOptional(candidate.model)"
                    />
                    <button
                      type="button"
                      title="목록에서 선택"
                      aria-label="모델을 목록에서 선택"
                      @click="customModel = { ...customModel, [key(candidate)]: false }"
                    ><Icon icon="lucide:list" /></button>
                  </span>
                </label>
                <label v-if="effortOptions[candidate.agent].length"
                  >노력치
                  <select
                    v-model="candidate.effort"
                    @change="candidate.effort = normalizeOptional(candidate.effort)"
                  >
                    <option value="">기본</option>
                    <option v-for="effort in effortOptions[candidate.agent]" :key="effort" :value="effort">{{ effort }}</option>
                  </select>
                </label>
                <label v-if="modeOptions[candidate.agent].length"
                  >모드
                  <select
                    v-model="candidate.mode"
                    @change="candidate.mode = normalizeOptional(candidate.mode)"
                  >
                    <option value="">기본</option>
                    <option v-for="mode in modeOptions[candidate.agent]" :key="mode" :value="mode">{{ mode }}</option>
                  </select>
                </label>
              </div>
            </details>
          </div>
          <p v-if="!draft.candidates.some(c => c.enabled === zone.enabled)" class="empty">
            {{ zone.enabled ? '사용할 프로필을 추가하세요' : '대기 프로필 없음' }}
          </p>
        </div>
      </div>
    </section>
    <p>
      최초 실행은 활성 순서를 따르며, 사용할 수 없는 프로필은 건너뜁니다. 이후 자동 전환은 선택
      전략을 따릅니다. 핸들에 초점을 두고 ↑ / ↓ 키로도 순서를 바꿀 수 있습니다.
    </p>
    <div class="save-actions">
      <button type="submit" :disabled="busy">{{ busy ? '저장 중…' : '저장' }}</button>
      <p role="status">{{ message || state.error }}</p>
    </div>
  </form>
</template>
<style scoped>
.loop-settings {
  display: flex;
  flex-direction: column;
  gap: 24px;
  min-width: 0;
  color: #d4d4d4;
}
h2 {
  margin: 0 0 6px;
  font-size: 20px;
  font-weight: 600;
  color: #e6e6e6;
}
h3 {
  margin: 0 0 16px;
  font-size: 14.5px;
  font-weight: 600;
  color: #e6e6e6;
}
.loop-settings p {
  margin: 0;
  max-width: 80ch;
  color: #aaaaaa;
  font-size: 13px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}
.card {
  min-width: 0;
  background: #202020;
  border: 1px solid #2b2b2b;
  border-radius: 10px;
  padding: 24px;
}
.thresholds {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 18px 20px;
}
.thresholds label {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.thresholds input,
.thresholds select {
  width: 100%;
}
.save-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.candidate {
  padding: 16px 0;
  border-top: 1px solid #333333;
}
.candidate:last-child {
  padding-bottom: 0;
}
.candidate label {
  display: flex;
  align-items: center;
  gap: 8px;
}
select,
input[type='number'] {
  box-sizing: border-box;
  width: 84px;
  min-width: 0;
  min-height: 40px;
  padding: 9px 12px;
  color: #e6e6e6;
  background: #252525;
  border: 1px solid #333333;
  border-radius: 6px;
  font: inherit;
  font-size: 13px;
}
input[type='text'],
select {
  width: 140px;
}
.thresholds input {
  width: 100%;
}
input[type='number']:focus {
  outline: none;
  border-color: var(--accent);
}
input[type='checkbox'] {
  width: 16px;
  height: 16px;
  margin: 0;
  flex-shrink: 0;
  accent-color: var(--accent);
}
button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 38px;
  flex-shrink: 0;
  background: #202020;
  color: #d4d4d4;
  border: 1px solid #333333;
  padding: 7px 14px;
  border-radius: 6px;
  font: inherit;
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
}
button:hover:not(:disabled) {
  background: #2a2a2a;
}
button:disabled {
  color: #555555;
  cursor: not-allowed;
}
.loop-settings :is(button, input):focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
label {
  font-size: 13px;
  color: #bbbbbb;
}
@media (max-width: 760px) {
  .card {
    padding: 18px;
  }
  .thresholds {
    grid-template-columns: minmax(0, 1fr);
  }
}

.profile-board {
  padding: 0;
  overflow: hidden;
}
.profile-zone {
  padding: 22px;
  min-height: 140px;
}
.profile-zone + .profile-zone {
  border-top: 1px solid #3b3b3b;
  background: #181818;
}
.profile-zone h3 {
  margin-bottom: 4px;
}
.profile-list {
  display: grid;
  gap: 8px;
  margin-top: 16px;
  min-width: 0;
}
.provider-tag {
  flex-shrink: 0;
  border: 1px solid #ffffff25;
  border-radius: 4px;
  padding: 2px 6px;
  font-size: 10px;
  line-height: 1.5;
  color: #bbb;
  background: #ffffff08;
}
.provider-tag.codex {
  color: #8ec6ef;
}
.provider-tag.claude {
  color: #d9b79d;
}
.candidate {
  display: block;
  padding: 10px;
  border: 1px solid #383838;
  border-radius: 7px;
  background: #242424;
}
.candidate:last-child {
  padding-bottom: 10px;
}
.candidate.dragging {
  opacity: 0.45;
}
.profile-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.profile-name {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
  font-size: 13px;
}
.drag-handle {
  cursor: grab;
  padding: 4px;
  min-height: 28px;
  border: 0;
  background: transparent;
  font-size: 18px;
}
.drag-handle:active {
  cursor: grabbing;
}
.order {
  font-size: 11px;
  color: var(--accent);
  min-width: 16px;
}
.toggle-profile {
  padding: 4px 6px;
  min-height: 28px;
  font-size: 11px;
}
summary {
  cursor: pointer;
  color: #aaa;
  font-size: 11px;
  margin: 8px 0 0 30px;
}
.profile-options {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  padding-top: 10px;
}
.profile-options label {
  display: grid;
  gap: 4px;
  font-size: 11px;
}
/*
 * The window that is not this account's basis keeps its saved value — flipping
 * the basis back has to bring the number the user had — so it dims rather than
 * disappearing, and the note under the control says what it still does.
 */
.profile-options label.inactive {
  opacity: 0.5;
}
.basis-note {
  margin: 0;
  color: #888;
  font-size: 11px;
  line-height: 1.6;
  text-wrap: pretty;
}
.basis-note strong {
  color: #aaa;
  font-weight: 600;
}
.basis-field {
  display: grid;
  gap: 4px;
  align-content: start;
  font-size: 11px;
}
.basis-label {
  color: #aaa;
}
.basis {
  display: flex;
  border: 1px solid #ffffff30;
  border-radius: 5px;
  overflow: hidden;
  width: fit-content;
}
.basis button {
  display: flex;
  align-items: center;
  gap: 5px;
  min-height: 30px;
  padding: 5px 11px;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: var(--text-secondary, #aeb3ba);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}
.basis button + button {
  border-left: 1px solid #ffffff20;
}
.basis button.on {
  background: var(--accent-softer, rgba(90, 155, 255, 0.22));
  color: var(--accent-strong, #82b4ff);
  font-weight: 600;
}
.basis-field small {
  max-width: 250px;
  color: #888;
  font-size: 10px;
  line-height: 1.5;
}
.profile-options input,
.profile-options select {
  width: 110px;
  min-height: 30px;
  padding: 5px;
}
.empty {
  padding: 18px 8px;
  border: 1px dashed #444;
  border-radius: 6px;
  text-align: center;
}
@media (max-width: 600px) {
  .profile-zone {
    padding: 12px;
  }
  .profile-row {
    flex-wrap: wrap;
  }
  .toggle-profile {
    margin-left: auto;
  }
}

.drag-handle {
  touch-action: none;
  user-select: none;
  -webkit-user-drag: none;
}
.drag-handle :deep(svg) {
  pointer-events: none;
}
.candidate,
.profile-list {
  position: relative;
}
.candidate.drop-before::before,
.drop-end .profile-list::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  height: 3px;
  background: var(--accent);
  border-radius: 2px;
  pointer-events: none;
}
.candidate.drop-before::before {
  top: -5px;
}
.drop-end .profile-list::after {
  bottom: -5px;
}
.model-custom {
  display: flex;
  gap: 4px;
  min-width: 0;
}
.model-custom input {
  flex: 1;
  min-width: 0;
}
.model-custom button {
  flex-shrink: 0;
  display: grid;
  place-items: center;
  width: 28px;
  color: inherit;
  background: #ffffff08;
  border: 1px solid #ffffff30;
  border-radius: 4px;
  cursor: pointer;
}
.model-custom button:hover {
  background: #ffffff14;
}
</style>
