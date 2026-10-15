<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { t } from "../composables/useI18n";
import { ENV_PRESETS, envError } from "../lib/account-env";
import type { CliAgentKind } from "../lib/persistence";

const props = defineProps<{ agent: CliAgentKind; modelValue?: Record<string, string>; reserved?: string[] }>();
const emit = defineEmits<{ save: [env: Record<string, string>] }>();
let nextId = 0;
const rows = ref<Array<{ id: number; name: string; value: string; visible: boolean }>>([]);
const saved = ref(false);
function reset() {
  rows.value = Object.entries(props.modelValue ?? {}).map(([name, value]) => ({ id: nextId++, name, value, visible: false }));
  saved.value = false;
}
watch(() => props.modelValue, reset, { immediate: true, deep: true });
const error = computed(() => envError(rows.value, props.reserved));
const dirty = computed(() => JSON.stringify(rows.value.map(r => [r.name, r.value])) !== JSON.stringify(Object.entries(props.modelValue ?? {})));
function add(name = "") {
  rows.value.push({ id: nextId++, name, value: "", visible: false });
  saved.value = false;
}
async function save() {
  if (error.value) return;
  emit("save", Object.fromEntries(rows.value.map(r => [r.name.trim(), r.value])));
  await nextTick();
  saved.value = true;
}
</script>

<template>
  <details class="env-editor">
    <summary>{{ t('Environment variables') }} <span class="count">{{ Object.keys(modelValue ?? {}).length }}</span><span v-if="dirty" class="pending">{{ t('Unsaved changes') }}</span></summary>
    <div class="env-content">
      <p>{{ t('Applied to new sessions after saving. Empty values override inherited values; remove a row to inherit again.') }}</p>
      <div class="presets" :aria-label="t('Quick add')">
        <button v-for="name in ENV_PRESETS[agent]" :key="name" type="button" :disabled="rows.some(r => r.name.trim().toUpperCase() === name)" @click="add(name)">+ {{ name }}</button>
      </div>
      <div v-for="row in rows" :key="row.id" class="env-row">
        <label>{{ t('Variable name') }}<input v-model="row.name" spellcheck="false" autocomplete="off" placeholder="MY_VARIABLE" @input="saved = false" /></label>
        <label>{{ t('Value') }}<input v-model="row.value" :type="row.visible ? 'text' : 'password'" spellcheck="false" autocomplete="off" :placeholder="t('Empty value')" @input="saved = false" /></label>
        <div class="row-tools">
          <button type="button" :aria-label="t(row.visible ? 'Hide value' : 'Show value')" :aria-pressed="row.visible" @click="row.visible = !row.visible">{{ t(row.visible ? 'Hide' : 'Show') }}</button>
          <button type="button" :aria-label="t('Remove variable') + ': ' + row.name" @click="rows = rows.filter(r => r.id !== row.id); saved = false">{{ t('Remove') }}</button>
        </div>
      </div>
      <p v-if="error" class="error" role="alert">{{ t(error) }}</p>
      <div class="footer">
        <button type="button" @click="add()">+ {{ t('Custom variable') }}</button>
        <span class="spacer" />
        <span v-if="saved && !dirty" role="status">{{ t('Saved') }}</span>
        <button type="button" :disabled="!dirty" @click="reset">{{ t('Cancel') }}</button>
        <button type="button" class="save" :disabled="!!error || !dirty" @click="save">{{ t('Save') }}</button>
      </div>
      <p class="storage-note">{{ t('Values are stored locally in app settings, without encryption.') }}</p>
    </div>
  </details>
</template>

<style scoped>
.env-editor { grid-area: env; min-width: 0; border-top: 1px solid #333; font-size: 12px; }
summary { cursor: pointer; padding: 12px 0 0; color: #ccc; }
.count { margin-left: 6px; color: #999; font-variant-numeric: tabular-nums; }
.pending { margin-left: 12px; color: #d7ba7d; }
.env-content { display: grid; gap: 12px; padding-top: 12px; }
p { margin: 0; color: #aaa; line-height: 1.6; }
.presets, .footer, .row-tools { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
button { background: #2a2a2a; color: #ddd; border: 1px solid #444; border-radius: 5px; padding: 6px 9px; cursor: pointer; font: inherit; }
button:hover:not(:disabled) { background: #383838; }
button:disabled { opacity: .4; cursor: default; }
button:focus-visible, summary:focus-visible, input:focus { outline: 1px solid var(--accent); outline-offset: 2px; }
.presets button { font: 11px Consolas, monospace; }
.env-row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 8px; }
label { display: grid; gap: 5px; color: #aaa; min-width: 0; }
input { box-sizing: border-box; width: 100%; min-width: 0; padding: 9px; background: #252525; border: 1px solid #444; border-radius: 5px; color: #e6e6e6; font: 12px Consolas, monospace; }
.row-tools { grid-column: 1 / -1; justify-content: flex-end; }
.spacer { flex: 1; }
.save { color: var(--accent); border-color: var(--accent); }
.error { color: #f48771; }
.storage-note { font-size: 11px; color: #999; }
</style>
