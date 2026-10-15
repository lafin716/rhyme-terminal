<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { Icon } from '@iconify/vue';
import { useLoopRouting, saveLoopSettings } from '../composables/useLoopRouting';
import {
  LOOP_THRESHOLD_BASES,
  loopBasisLabel,
  mergeLoopProfiles,
  type LoopAgent,
  type LoopSettings,
  type LoopCandidate,
} from '../lib/loop-routing';
import { useAccountProfiles } from '../composables/useAccountProfiles';
import { sessionAgentIcon } from '../lib/session-agent-icon';

const { state } = useLoopRouting();
const clone = () =>
  JSON.parse(JSON.stringify(mergeLoopProfiles(state.settings, useAccountProfiles().profiles))) as LoopSettings;
const draft = ref(clone());
const busy = ref(false);
const message = ref('');
const messageKind = ref<'ok' | 'error'>('ok');
let messageTimer: ReturnType<typeof setTimeout> | undefined;

watch(
  () => useAccountProfiles().profiles,
  () => {
    endDrag();
    draft.value = mergeLoopProfiles(draft.value, useAccountProfiles().profiles);
  },
  { deep: true },
);
const zones = [
  {
    enabled: true,
    title: '활성 프로필',
    icon: 'lucide:circle-play',
    hint: '위에서부터 순서대로 시도합니다. 핸들을 끌거나, 핸들에 초점을 두고 ↑ / ↓ 로 순서를 바꾸세요.',
  },
  {
    enabled: false,
    title: '대기 프로필',
    icon: 'lucide:circle-pause',
    hint: '루프에 참여하지 않습니다. 활성 영역으로 끌어오거나 [활성화]를 누르세요.',
  },
];
const active = computed(() => draft.value.candidates.filter(c => c.enabled));
const inZone = (enabled: boolean) => draft.value.candidates.filter(c => c.enabled === enabled);
const dragged = ref<LoopCandidate | null>(null);
const key = (c: LoopCandidate) => c.agent + ':' + (c.profileId ?? 'system');
const fieldId = (c: LoopCandidate, name: string) => `loop-${c.agent}-${c.profileId ?? 'system'}-${name}`;
const agentLabel = (agent: LoopAgent) => (agent === 'claude' ? 'Claude Code' : 'Codex');
const dropTarget = ref<{ enabled: boolean; before?: string } | null>(null);
/** Collapsed by default: the row's summary chips already say how a profile is set up. */
const expanded = ref<Record<string, boolean>>({});
const toggleExpanded = (c: LoopCandidate) =>
  (expanded.value = { ...expanded.value, [key(c)]: !expanded.value[key(c)] });
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
const strategyOptions = [
  { value: 'SMART', label: 'SMART', hint: '사용량이 적은 계정부터. 같으면 우선순위가 높은 쪽' },
  { value: 'ROUND_ROBIN', label: 'ROUND ROBIN', hint: '활성 목록 순서대로 바로 다음 계정' },
  { value: 'LEAST_USAGE', label: 'LEAST USAGE', hint: '남은 한도가 가장 넉넉한 계정' },
  { value: 'PRIORITY', label: 'PRIORITY', hint: '우선순위 값이 큰 계정부터' },
];
const strategyHint = computed(
  () => strategyOptions.find(s => s.value === (draft.value.strategy ?? 'SMART'))?.hint ?? '',
);
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

/*
 * Every per-profile override, each resettable on its own. A field at its
 * default is not an override: the thresholds fall back to the global numbers
 * and the rest to whatever the CLI picks by itself, so 초기화 here hands one
 * decision back rather than writing a value.
 */
type OverrideField = 'basis' | 'short' | 'weekly' | 'priority' | 'model' | 'effort' | 'mode';
const OVERRIDE_FIELDS: OverrideField[] = ['basis', 'short', 'weekly', 'priority', 'model', 'effort', 'mode'];
const basisOf = (c: LoopCandidate) => c.thresholdBasis ?? 'short';
function isOverridden(c: LoopCandidate, field: OverrideField): boolean {
  switch (field) {
    case 'basis':
      return basisOf(c) !== 'short';
    case 'short':
      return c.shortThreshold != null;
    case 'weekly':
      return c.weeklyThreshold != null;
    case 'priority':
      return (c.priority ?? 0) !== 0;
    case 'model':
      return !!c.model;
    case 'effort':
      return !!c.effort;
    case 'mode':
      return !!c.mode;
  }
}
function resetField(c: LoopCandidate, field: OverrideField) {
  switch (field) {
    case 'basis':
      c.thresholdBasis = 'short';
      break;
    case 'short':
      c.shortThreshold = null;
      break;
    case 'weekly':
      c.weeklyThreshold = null;
      break;
    case 'priority':
      c.priority = 0;
      break;
    case 'model':
      c.model = null;
      customModel.value = { ...customModel.value, [key(c)]: false };
      break;
    case 'effort':
      c.effort = null;
      break;
    case 'mode':
      c.mode = null;
      break;
  }
}
const overrideCount = (c: LoopCandidate) => OVERRIDE_FIELDS.filter(field => isOverridden(c, field)).length;
function resetCandidate(c: LoopCandidate) {
  for (const field of OVERRIDE_FIELDS) resetField(c, field);
}

const effectiveThreshold = (c: LoopCandidate) =>
  basisOf(c) === 'weekly'
    ? c.weeklyThreshold ?? draft.value.weeklyThreshold
    : c.shortThreshold ?? draft.value.shortThreshold;
/** What a collapsed row says about a profile, so it need not be opened to be read. */
function chips(c: LoopCandidate) {
  const rows = [
    {
      icon: basisOf(c) === 'weekly' ? 'lucide:calendar-range' : 'lucide:clock',
      text: `${loopBasisLabel(basisOf(c))} ${effectiveThreshold(c)}%`,
      custom: isOverridden(c, 'basis') || isOverridden(c, basisOf(c) === 'weekly' ? 'weekly' : 'short'),
    },
  ];
  if (c.model) rows.push({ icon: 'lucide:box', text: c.model, custom: true });
  if (c.effort) rows.push({ icon: 'lucide:gauge', text: c.effort, custom: true });
  if (c.mode) rows.push({ icon: 'lucide:shield-check', text: c.mode, custom: true });
  if ((c.priority ?? 0) !== 0)
    rows.push({ icon: 'lucide:arrow-up-narrow-wide', text: `우선순위 ${c.priority}`, custom: true });
  return rows;
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
onBeforeUnmount(() => {
  endDrag();
  clearTimeout(messageTimer);
});
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
/**
 * Only the fields this page edits, in order, so the unsaved marker survives the
 * round trip through `saveLoopSettings` — which rebuilds every candidate and
 * refills `env` from the account profiles.
 */
const signature = (settings: LoopSettings) =>
  JSON.stringify([
    settings.strategy ?? 'SMART',
    settings.pollingIntervalSeconds ?? null,
    settings.shortThreshold,
    settings.weeklyThreshold,
    settings.candidates.map(c => [
      key(c),
      c.enabled,
      c.priority ?? 0,
      c.shortThreshold ?? null,
      c.weeklyThreshold ?? null,
      basisOf(c),
      c.model ?? null,
      c.effort ?? null,
      c.mode ?? null,
    ]),
  ]);
const dirty = computed(
  () => signature(draft.value) !== signature(mergeLoopProfiles(state.settings, useAccountProfiles().profiles)),
);
function note(text: string, kind: 'ok' | 'error') {
  message.value = text;
  messageKind.value = kind;
  clearTimeout(messageTimer);
  if (kind === 'ok') messageTimer = setTimeout(() => (message.value = ''), 2500);
}
function discard() {
  endDrag();
  draft.value = clone();
  customModel.value = {};
  note('저장한 내용으로 되돌렸습니다.', 'ok');
}
async function save() {
  busy.value = true;
  message.value = '';
  try {
    await saveLoopSettings(draft.value);
    note('저장했습니다.', 'ok');
  } catch (e) {
    note(String(e), 'error');
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <form class="loop-settings" @submit.prevent="save">
    <header class="page-head">
      <div class="page-head-text">
        <h2>에이전트 루프</h2>
        <p>활성 순서의 첫 프로필로 새 루프가 바로 시작합니다. 한도에 닿으면 여기서 정한 규칙대로 다음 계정이 이어받습니다.</p>
      </div>
      <div class="page-head-actions">
        <span v-if="message" class="save-note" :class="messageKind" role="status">
          <Icon :icon="messageKind === 'error' ? 'lucide:triangle-alert' : 'lucide:check'" />{{ message }}
        </span>
        <span v-else-if="dirty" class="save-note dirty" role="status">
          <Icon icon="lucide:pencil-line" />저장하지 않은 변경
        </span>
        <button type="button" class="btn" :disabled="busy || !dirty" @click="discard">
          <Icon icon="lucide:undo-2" />되돌리기
        </button>
        <button type="submit" class="btn primary" :disabled="busy">
          <Icon :icon="busy ? 'lucide:loader-circle' : 'lucide:save'" :class="{ spin: busy }" />{{ busy ? '저장 중…' : '저장' }}
        </button>
      </div>
    </header>

    <p v-if="state.error" class="banner" role="alert"><Icon icon="lucide:triangle-alert" />{{ state.error }}</p>

    <section class="card">
      <div class="card-head">
        <Icon icon="lucide:sliders-horizontal" />
        <h3>전역 기준</h3>
        <span class="card-head-hint">모든 프로필의 기본값</span>
      </div>
      <div class="field-grid">
        <div class="field">
          <label for="loop-short">5시간 한도 제한</label>
          <div class="input-wrap">
            <input id="loop-short" v-model.number="draft.shortThreshold" type="number" min="1" max="100" required />
            <span class="suffix">%</span>
          </div>
          <small>5시간 창이 이 비율에 닿으면 다음 계정으로 넘깁니다</small>
        </div>
        <div class="field">
          <label for="loop-weekly">주간 한도 제한</label>
          <div class="input-wrap">
            <input id="loop-weekly" v-model.number="draft.weeklyThreshold" type="number" min="1" max="100" required />
            <span class="suffix">%</span>
          </div>
          <small>주간 창이 이 비율에 닿으면 다음 계정으로 넘깁니다</small>
        </div>
        <div class="field">
          <label for="loop-strategy">전환 순서</label>
          <select id="loop-strategy" v-model="draft.strategy">
            <option v-for="option in strategyOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
          </select>
          <small>{{ strategyHint }}</small>
        </div>
        <div class="field">
          <label for="loop-polling">사용량 조회 간격</label>
          <div class="input-wrap">
            <input id="loop-polling" v-model.number="draft.pollingIntervalSeconds" type="number" min="60" max="300" />
            <span class="suffix">초</span>
          </div>
          <small>프롬프트가 실행 중인 계정만 조회합니다. 대기 중인 계정은 사용량이 움직이지 않습니다</small>
        </div>
      </div>
      <p class="callout">
        <Icon icon="lucide:info" />
        <span>
          프로필마다 <strong>개별 기준</strong>에서 이 값을 덮어쓸 수 있고, 비워 두면 위 값을 그대로 씁니다.
          전환을 결정하는 창은 계정마다 <strong>사용량 기준</strong>에서 고르며 기본값은 5시간입니다.
          기준이 아닌 창은 100% 소진에서만 막습니다.
        </span>
      </p>
    </section>

    <section class="card profile-board" aria-label="전체 에이전트 프로필">
      <div
        v-for="zone in zones"
        :key="String(zone.enabled)"
        class="profile-zone"
        :data-enabled="zone.enabled"
        :class="{ 'drop-end': dropTarget?.enabled === zone.enabled && !dropTarget.before }"
      >
        <div class="zone-head">
          <h3><Icon :icon="zone.icon" />{{ zone.title }}<span class="count">{{ inZone(zone.enabled).length }}</span></h3>
          <p>{{ zone.hint }}</p>
        </div>
        <div class="profile-list">
          <div
            v-for="candidate in inZone(zone.enabled)"
            :key="key(candidate)"
            class="candidate"
            :data-profile-key="key(candidate)"
            :class="{
              dragging: dragged === candidate,
              'drop-before': dropTarget?.before === key(candidate),
              open: expanded[key(candidate)],
            }"
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
              <span class="agent-badge" :class="candidate.agent" aria-hidden="true">
                <Icon :icon="sessionAgentIcon(candidate.agent)" />
              </span>
              <span class="profile-id">
                <span class="profile-name">{{ candidate.label }}</span>
                <span class="provider-tag" :class="candidate.agent">{{ agentLabel(candidate.agent) }}</span>
              </span>
              <span class="row-actions">
                <button
                  type="button"
                  class="toggle-profile"
                  :class="{ activate: !zone.enabled }"
                  @click="place(candidate, !zone.enabled)"
                >
                  <Icon :icon="zone.enabled ? 'lucide:circle-pause' : 'lucide:circle-play'" />{{ zone.enabled ? '대기로' : '활성화' }}
                </button>
                <button
                  type="button"
                  class="expand"
                  :aria-expanded="!!expanded[key(candidate)]"
                  :aria-controls="fieldId(candidate, 'options')"
                  @click="toggleExpanded(candidate)"
                >
                  <Icon :icon="expanded[key(candidate)] ? 'lucide:chevron-up' : 'lucide:chevron-down'" />개별 기준
                  <span v-if="overrideCount(candidate)" class="override-count">{{ overrideCount(candidate) }}</span>
                </button>
              </span>
            </div>

            <div v-if="!expanded[key(candidate)]" class="summary-chips">
              <span v-for="chip in chips(candidate)" :key="chip.text" class="chip" :class="{ custom: chip.custom }">
                <Icon :icon="chip.icon" />{{ chip.text }}
              </span>
            </div>

            <div v-show="expanded[key(candidate)]" :id="fieldId(candidate, 'options')" class="profile-options">
              <div class="options-head">
                <span class="options-title">개별 기준</span>
                <span class="options-hint">비우거나 초기화하면 전역 기준을 따릅니다</span>
                <button
                  type="button"
                  class="reset-all"
                  :disabled="!overrideCount(candidate)"
                  :title="candidate.label + ' 개별 기준을 모두 기본값으로'"
                  @click="resetCandidate(candidate)"
                >
                  <Icon icon="lucide:rotate-ccw" />모두 초기화<span v-if="overrideCount(candidate)"> ({{ overrideCount(candidate) }})</span>
                </button>
              </div>
              <div class="options-grid">
                <div class="field wide">
                  <div class="field-head">
                    <span class="field-label">사용량 기준</span>
                    <button
                      v-if="isOverridden(candidate, 'basis')"
                      type="button"
                      class="reset-field"
                      title="기본값(5시간)으로 초기화"
                      aria-label="사용량 기준 초기화"
                      @click="resetField(candidate, 'basis')"
                    ><Icon icon="lucide:rotate-ccw" /></button>
                  </div>
                  <div class="basis" role="group" :aria-label="candidate.label + ' 사용량 기준'">
                    <button
                      v-for="basis in LOOP_THRESHOLD_BASES"
                      :key="basis.value"
                      type="button"
                      :class="{ on: basisOf(candidate) === basis.value }"
                      :aria-pressed="basisOf(candidate) === basis.value"
                      :title="basis.hint"
                      @click="candidate.thresholdBasis = basis.value"
                    >
                      <Icon :icon="basis.value === 'weekly' ? 'lucide:calendar-range' : 'lucide:clock'" />{{ basis.label }}
                    </button>
                  </div>
                  <small>이 계정을 전환할 창입니다. 나머지 창은 100% 소진에서만 막습니다</small>
                </div>

                <div class="field" :class="{ inactive: basisOf(candidate) !== 'short' }">
                  <div class="field-head">
                    <label :for="fieldId(candidate, 'short')">5시간 한도</label>
                    <button
                      v-if="isOverridden(candidate, 'short')"
                      type="button"
                      class="reset-field"
                      title="전역 기준으로 초기화"
                      aria-label="5시간 한도 초기화"
                      @click="resetField(candidate, 'short')"
                    ><Icon icon="lucide:rotate-ccw" /></button>
                  </div>
                  <div class="input-wrap">
                    <input
                      :id="fieldId(candidate, 'short')"
                      v-model.number="candidate.shortThreshold"
                      type="number"
                      min="1"
                      max="100"
                      :placeholder="String(draft.shortThreshold)"
                      @change="candidate.shortThreshold = candidate.shortThreshold || null"
                    />
                    <span class="suffix">%</span>
                  </div>
                  <small>전역 {{ draft.shortThreshold }}%</small>
                </div>

                <div class="field" :class="{ inactive: basisOf(candidate) !== 'weekly' }">
                  <div class="field-head">
                    <label :for="fieldId(candidate, 'weekly')">주간 한도</label>
                    <button
                      v-if="isOverridden(candidate, 'weekly')"
                      type="button"
                      class="reset-field"
                      title="전역 기준으로 초기화"
                      aria-label="주간 한도 초기화"
                      @click="resetField(candidate, 'weekly')"
                    ><Icon icon="lucide:rotate-ccw" /></button>
                  </div>
                  <div class="input-wrap">
                    <input
                      :id="fieldId(candidate, 'weekly')"
                      v-model.number="candidate.weeklyThreshold"
                      type="number"
                      min="1"
                      max="100"
                      :placeholder="String(draft.weeklyThreshold)"
                      @change="candidate.weeklyThreshold = candidate.weeklyThreshold || null"
                    />
                    <span class="suffix">%</span>
                  </div>
                  <small>전역 {{ draft.weeklyThreshold }}%</small>
                </div>

                <div class="field">
                  <div class="field-head">
                    <label :for="fieldId(candidate, 'priority')">우선순위</label>
                    <button
                      v-if="isOverridden(candidate, 'priority')"
                      type="button"
                      class="reset-field"
                      title="기본값(0)으로 초기화"
                      aria-label="우선순위 초기화"
                      @click="resetField(candidate, 'priority')"
                    ><Icon icon="lucide:rotate-ccw" /></button>
                  </div>
                  <input :id="fieldId(candidate, 'priority')" v-model.number="candidate.priority" type="number" placeholder="0" />
                  <small>숫자가 클수록 먼저</small>
                </div>

                <div class="field">
                  <div class="field-head">
                    <label :for="fieldId(candidate, 'model')">모델</label>
                    <button
                      v-if="isOverridden(candidate, 'model')"
                      type="button"
                      class="reset-field"
                      title="에이전트 기본값으로 초기화"
                      aria-label="모델 초기화"
                      @click="resetField(candidate, 'model')"
                    ><Icon icon="lucide:rotate-ccw" /></button>
                  </div>
                  <select
                    v-if="!customModel[key(candidate)]"
                    :id="fieldId(candidate, 'model')"
                    :value="candidate.model ?? ''"
                    @change="onModelSelect(candidate, $event)"
                  >
                    <option value="">{{ candidate.agent }} 기본</option>
                    <option v-for="model in modelChoices(candidate)" :key="model" :value="model">{{ model }}</option>
                    <option :value="MODEL_CUSTOM">직접 입력…</option>
                  </select>
                  <span v-else class="model-custom">
                    <input
                      :id="fieldId(candidate, 'model')"
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
                  <small>루프가 이 계정으로 실행할 모델</small>
                </div>

                <div v-if="effortOptions[candidate.agent].length" class="field">
                  <div class="field-head">
                    <label :for="fieldId(candidate, 'effort')">노력치</label>
                    <button
                      v-if="isOverridden(candidate, 'effort')"
                      type="button"
                      class="reset-field"
                      title="에이전트 기본값으로 초기화"
                      aria-label="노력치 초기화"
                      @click="resetField(candidate, 'effort')"
                    ><Icon icon="lucide:rotate-ccw" /></button>
                  </div>
                  <select
                    :id="fieldId(candidate, 'effort')"
                    :value="candidate.effort ?? ''"
                    @change="candidate.effort = normalizeOptional(($event.target as HTMLSelectElement).value)"
                  >
                    <option value="">기본</option>
                    <option v-for="effort in effortOptions[candidate.agent]" :key="effort" :value="effort">{{ effort }}</option>
                  </select>
                  <small>--effort</small>
                </div>

                <div v-if="modeOptions[candidate.agent].length" class="field">
                  <div class="field-head">
                    <label :for="fieldId(candidate, 'mode')">모드</label>
                    <button
                      v-if="isOverridden(candidate, 'mode')"
                      type="button"
                      class="reset-field"
                      title="에이전트 기본값으로 초기화"
                      aria-label="모드 초기화"
                      @click="resetField(candidate, 'mode')"
                    ><Icon icon="lucide:rotate-ccw" /></button>
                  </div>
                  <select
                    :id="fieldId(candidate, 'mode')"
                    :value="candidate.mode ?? ''"
                    @change="candidate.mode = normalizeOptional(($event.target as HTMLSelectElement).value)"
                  >
                    <option value="">기본</option>
                    <option v-for="mode in modeOptions[candidate.agent]" :key="mode" :value="mode">{{ mode }}</option>
                  </select>
                  <small>--permission-mode</small>
                </div>
              </div>
            </div>
          </div>
          <p v-if="!inZone(zone.enabled).length" class="empty">
            <Icon :icon="zone.enabled ? 'lucide:circle-play' : 'lucide:inbox'" />
            {{ zone.enabled ? '활성 프로필이 없습니다. 아래에서 활성화하세요' : '대기 프로필 없음' }}
          </p>
        </div>
      </div>
    </section>

    <section class="card">
      <div class="card-head">
        <Icon icon="lucide:route" />
        <h3>동작 방식</h3>
      </div>
      <ul class="how">
        <li><Icon icon="lucide:play" /><span>최초 실행은 <strong>활성 순서</strong>를 따르고, 한도에 걸린 프로필은 건너뜁니다.</span></li>
        <li><Icon icon="lucide:shuffle" /><span>이후 전환은 위에서 고른 <strong>전환 순서</strong>를 따릅니다.</span></li>
        <li><Icon icon="lucide:hourglass" /><span>모든 프로필이 한도에 걸리면 가장 먼저 초기화되는 시각까지 기다렸다가 다시 시도합니다.</span></li>
        <li><Icon icon="lucide:keyboard" /><span>핸들에 초점을 두고 ↑ / ↓ 키로도 순서를 바꿀 수 있습니다.</span></li>
      </ul>
    </section>
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
.loop-settings * {
  box-sizing: border-box;
}

/* Pinned to the top of the settings scroller so 저장 is always one click away. */
.page-head {
  position: sticky;
  top: 0;
  z-index: 3;
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px 20px;
  margin: -32px -32px 0;
  padding: 28px 32px 16px;
  background: #1e1e1e;
  border-bottom: 1px solid #2b2b2b;
}
.page-head-text {
  flex: 1 1 320px;
  min-width: 0;
}
.page-head-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  padding-top: 2px;
}
h2 {
  margin: 0 0 6px;
  font-size: 20px;
  font-weight: 600;
  color: #e6e6e6;
}
h3 {
  margin: 0;
  font-size: 14.5px;
  font-weight: 600;
  color: #e6e6e6;
}
.loop-settings p {
  margin: 0;
  max-width: 82ch;
  color: #aaaaaa;
  font-size: 13px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}
.save-note {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border-radius: 20px;
  border: 1px solid #2b2b2b;
  background: #1b1b1b;
  color: #aaaaaa;
  font-size: 12px;
}
.save-note.ok {
  color: #7fca8d;
  border-color: rgba(127, 202, 141, 0.32);
  background: rgba(127, 202, 141, 0.1);
}
.save-note.dirty {
  color: var(--accent-strong);
  border-color: var(--accent-border);
  background: var(--accent-soft);
}
.save-note.error {
  color: #e98686;
  border-color: rgba(233, 134, 134, 0.34);
  background: rgba(233, 134, 134, 0.1);
}
.banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-radius: 8px;
  color: #e98686 !important;
  background: rgba(233, 134, 134, 0.09);
  border: 1px solid rgba(233, 134, 134, 0.28);
  font-size: 13px;
}

.card {
  min-width: 0;
  background: #202020;
  border: 1px solid #2b2b2b;
  border-radius: 10px;
  padding: 24px;
}
.card-head {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-bottom: 18px;
  color: #8a8a8a;
  font-size: 15px;
}
.card-head-hint {
  margin-left: auto;
  color: #808080;
  font-size: 12px;
}

.field-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 230px), 1fr));
  gap: 20px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 7px;
  min-width: 0;
}
.field > label,
.field-label {
  font-size: 12.5px;
  font-weight: 500;
  color: #c8c8c8;
}
.field small {
  color: #858585;
  font-size: 11.5px;
  line-height: 1.55;
  text-wrap: pretty;
}
.field-head {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 20px;
}
.input-wrap {
  position: relative;
  display: flex;
  align-items: center;
  min-width: 0;
}
.input-wrap input {
  width: 100%;
  padding-right: 34px;
}
.input-wrap .suffix {
  position: absolute;
  right: 11px;
  color: #808080;
  font-size: 12px;
  pointer-events: none;
}
input,
select {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 38px;
  padding: 9px 11px;
  color: #e6e6e6;
  background: #252525;
  border: 1px solid #333333;
  border-radius: 6px;
  font: inherit;
  font-size: 13px;
}
input::placeholder {
  color: #6b6b6b;
}
input:focus,
select:focus {
  outline: none;
  border-color: var(--accent);
}
.callout {
  display: flex;
  gap: 9px;
  margin-top: 20px !important;
  padding: 13px 14px;
  border-radius: 8px;
  background: #1a1a1a;
  border: 1px solid #2b2b2b;
  color: #8f8f8f !important;
  font-size: 12px !important;
  line-height: 1.7 !important;
}
.callout :deep(svg) {
  flex-shrink: 0;
  margin-top: 2px;
  font-size: 14px;
}
.callout strong {
  color: #c0c0c0;
  font-weight: 600;
}

button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 36px;
  flex-shrink: 0;
  background: #262626;
  color: #d4d4d4;
  border: 1px solid #333333;
  padding: 7px 13px;
  border-radius: 6px;
  font: inherit;
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
}
button:hover:not(:disabled) {
  background: #2f2f2f;
  border-color: #3d3d3d;
}
button:disabled {
  color: #5a5a5a;
  cursor: not-allowed;
}
.btn.primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--accent-on);
  font-weight: 600;
}
.btn.primary:hover:not(:disabled) {
  background: var(--accent-strong);
  border-color: var(--accent-strong);
}
.btn.primary:disabled {
  opacity: 0.55;
  color: var(--accent-on);
}
.loop-settings :is(button, input, select):focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.spin {
  animation: loop-spin 1s linear infinite;
}
@keyframes loop-spin {
  to {
    transform: rotate(360deg);
  }
}

.profile-board {
  padding: 0;
  overflow: hidden;
}
.profile-zone {
  padding: 22px;
  min-height: 132px;
}
.profile-zone + .profile-zone {
  border-top: 1px solid #2b2b2b;
  background: #1b1b1b;
}
.zone-head h3 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 5px;
}
.zone-head h3 :deep(svg) {
  color: #8a8a8a;
  font-size: 15px;
}
.zone-head p {
  color: #8a8a8a;
  font-size: 12px;
  line-height: 1.6;
}
.count {
  min-width: 20px;
  padding: 1px 7px;
  border-radius: 20px;
  background: #2a2a2a;
  color: #b4b4b4;
  font-size: 11px;
  font-weight: 600;
  text-align: center;
}
.profile-zone[data-enabled='true'] .count {
  background: var(--accent-soft);
  color: var(--accent-strong);
}
.profile-list {
  display: grid;
  gap: 8px;
  margin-top: 16px;
  min-width: 0;
}

.candidate {
  display: block;
  padding: 10px 12px;
  border: 1px solid #333333;
  border-radius: 8px;
  background: #242424;
}
.profile-zone[data-enabled='true'] .candidate {
  background: #262626;
}
.candidate.open {
  border-color: #3f3f3f;
  background: #232323;
}
.candidate.dragging {
  opacity: 0.45;
}
.profile-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.profile-id {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  flex-wrap: wrap;
}
.profile-name {
  min-width: 0;
  overflow-wrap: anywhere;
  font-size: 13.5px;
  font-weight: 500;
  color: #e6e6e6;
}
.provider-tag {
  flex-shrink: 0;
  border: 1px solid #ffffff20;
  border-radius: 4px;
  padding: 1px 6px;
  font-size: 10.5px;
  line-height: 1.6;
  color: #a5a5a5;
  background: #ffffff08;
}
.provider-tag.codex {
  color: #8ec6ef;
}
.provider-tag.claude {
  color: #d9b79d;
}
.agent-badge {
  width: 28px;
  height: 28px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
}
.agent-badge :deep(svg) {
  width: 19px;
  height: 19px;
}
.agent-badge.claude {
  background: rgba(217, 119, 87, 0.12);
  color: #d97757;
}
.agent-badge.codex {
  background: #303030;
  color: #e6e6e6;
}
.agent-badge.codex :deep(svg) {
  width: 24px;
  height: 24px;
}
.row-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}
.drag-handle {
  cursor: grab;
  padding: 3px;
  min-height: 26px;
  gap: 0;
  border: 0;
  background: transparent;
  color: #6f6f6f;
  font-size: 17px;
}
.drag-handle:hover:not(:disabled) {
  background: transparent;
  color: #b4b4b4;
}
.drag-handle:active {
  cursor: grabbing;
}
.order {
  flex-shrink: 0;
  min-width: 20px;
  height: 20px;
  display: grid;
  place-items: center;
  border-radius: 5px;
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-size: 11px;
  font-weight: 700;
}
.toggle-profile,
.expand {
  padding: 5px 9px;
  min-height: 30px;
  font-size: 11.5px;
  color: #b4b4b4;
}
.toggle-profile.activate {
  color: var(--accent-strong);
  border-color: var(--accent-border);
  background: var(--accent-soft);
}
.toggle-profile.activate:hover:not(:disabled) {
  background: var(--accent-softer);
  border-color: var(--accent);
}
.override-count {
  min-width: 16px;
  padding: 0 5px;
  border-radius: 20px;
  background: var(--accent-softer);
  color: var(--accent-strong);
  font-size: 10px;
  font-weight: 700;
  line-height: 16px;
}
.summary-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin: 8px 0 1px 36px;
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 8px;
  border-radius: 5px;
  border: 1px solid #333333;
  background: #1e1e1e;
  color: #9a9a9a;
  font-size: 11px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}
.chip.custom {
  border-color: var(--accent-border);
  color: var(--accent-strong);
  background: var(--accent-soft);
}
.chip :deep(svg) {
  flex-shrink: 0;
  font-size: 12px;
}

.profile-options {
  margin-top: 12px;
  padding: 14px;
  border-radius: 7px;
  background: #1c1c1c;
  border: 1px solid #2e2e2e;
}
.options-head {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}
.options-title {
  font-size: 12.5px;
  font-weight: 600;
  color: #d4d4d4;
}
.options-hint {
  flex: 1;
  min-width: 0;
  color: #7e7e7e;
  font-size: 11.5px;
}
.reset-all {
  padding: 4px 10px;
  min-height: 28px;
  font-size: 11.5px;
  color: #b4b4b4;
}
.reset-all:hover:not(:disabled) {
  color: var(--accent-strong);
  border-color: var(--accent-border);
}
.reset-field {
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  min-height: 20px;
  padding: 0;
  border-radius: 5px;
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-size: 11px;
}
.reset-field:hover:not(:disabled) {
  background: var(--accent-softer);
  border-color: var(--accent);
}
.options-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 178px), 1fr));
  gap: 16px;
  align-items: start;
}
.options-grid .field > label,
.options-grid .field-label {
  font-size: 11.5px;
  color: #bbbbbb;
}
.options-grid .field small {
  font-size: 11px;
}
.options-grid input,
.options-grid select {
  min-height: 32px;
  padding: 6px 9px;
  font-size: 12.5px;
}
.options-grid .input-wrap input {
  padding-right: 30px;
}
/* Its own row whatever the column count: `span 2` would force an implicit
 * second column once the grid narrows to one. */
.wide {
  grid-column: 1 / -1;
}
/*
 * The window that is not this account's basis keeps its saved value — flipping
 * the basis back has to bring the number the user had — so it dims rather than
 * disappearing, and the note under the control says what it still does.
 */
.field.inactive {
  opacity: 0.55;
}
.basis {
  display: flex;
  border: 1px solid #383838;
  border-radius: 6px;
  overflow: hidden;
  width: fit-content;
}
.basis button {
  gap: 5px;
  min-height: 32px;
  padding: 5px 12px;
  border: 0;
  border-radius: 0;
  background: #252525;
  color: #a5a5a5;
  font-size: 12px;
}
.basis button + button {
  border-left: 1px solid #383838;
}
.basis button.on {
  background: var(--accent-softer);
  color: var(--accent-strong);
  font-weight: 600;
}
.model-custom {
  display: flex;
  gap: 5px;
  min-width: 0;
}
.model-custom input {
  flex: 1;
  min-width: 0;
}
.model-custom button {
  flex-shrink: 0;
  width: 32px;
  min-height: 32px;
  padding: 0;
}
.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 22px 8px;
  border: 1px dashed #3a3a3a;
  border-radius: 8px;
  color: #7e7e7e !important;
  font-size: 12.5px !important;
  text-align: center;
}

.how {
  display: grid;
  gap: 11px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.how li {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  color: #a5a5a5;
  font-size: 12.5px;
  line-height: 1.65;
}
.how li :deep(svg) {
  flex-shrink: 0;
  margin-top: 2px;
  color: #6f6f6f;
  font-size: 14px;
}
.how strong {
  color: #d4d4d4;
  font-weight: 600;
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

@media (max-width: 720px) {
  .page-head {
    margin: -32px -20px 0;
    padding: 22px 20px 14px;
  }
  .card {
    padding: 18px;
  }
  .profile-zone {
    padding: 16px;
  }
  .profile-row {
    flex-wrap: wrap;
  }
  .row-actions {
    margin-left: auto;
  }
  .summary-chips {
    margin-left: 0;
  }
}
</style>
