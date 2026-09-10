<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { t } from "../composables/useI18n";
import { useKeybindings } from "../composables/useKeybindings";
import { captureKeybinding, captureWheelBinding, formatKeybinding, sameBinding, type ActionDef } from "../lib/keybindings";

const { actions, bindingFor, prefixFor, setBinding, setPrefix, resetBinding, resetPrefix, resetAll, isOverridden } = useKeybindings();
const capture = ref<{ id: string; prefix: boolean } | null>(null);
const categories: [ActionDef["category"], string][] = [
  ["terminal", "Terminal"], ["session", "Session"], ["pane", "Pane"],
  ["window", "Window"], ["editor", "Editor"], ["settings", "Settings"],
];
const groups = categories.map(([id, label]) => ({ id, label, actions: actions.filter(a => a.category === id) }));
const conflicts = computed(() => {
  const ids = new Set<string>();
  for (let i = 0; i < actions.length; i++) {
    for (const b of actions.slice(i + 1)) {
      const a = actions[i];
      // Different focused surfaces can safely use the same binding.
      const overlap = !a.scope || !b.scope || a.scope === b.scope || a.scope === "prefix" || b.scope === "prefix";
      if (overlap && bindingFor(a.id)?.key && sameBinding(bindingFor(a.id), bindingFor(b.id))) {
        ids.add(a.id); ids.add(b.id);
      }
      if (!a.scope && !b.scope && prefixFor(a.id) && prefixFor(a.id) === prefixFor(b.id)) {
        ids.add(a.id); ids.add(b.id);
      }
    }
  }
  return ids;
});

function start(id: string, prefix = false) {
  capture.value = capture.value?.id === id && capture.value.prefix === prefix ? null : { id, prefix };
}
function display(id: string, prefix = false) {
  if (capture.value?.id === id && capture.value.prefix === prefix) return t(id.startsWith("terminal.openLink") ? "Click with modifiers... (Esc to cancel)" : "Press a key... (Esc to cancel)");
  if (prefix) return prefixFor(id) ?? t("(unbound)");
  const binding = bindingFor(id);
  if (binding?.key?.startsWith("Wheel")) {
    return formatKeybinding(binding).replace("WheelUp", t("Wheel Up")).replace("WheelDown", t("Wheel Down"));
  }
  return t(formatKeybinding(binding));
}
function onKey(ev: KeyboardEvent) {
  if (!capture.value) return;
  ev.preventDefault();
  ev.stopImmediatePropagation();
  if (ev.key === "Escape") { capture.value = null; return; }
  if (ev.isComposing || ["Control", "Shift", "Alt", "Meta"].includes(ev.key)) return;
  if (capture.value.id.startsWith("terminal.openLink")) return;
  if (capture.value.prefix) {
    if (ev.ctrlKey || ev.altKey || ev.metaKey) return;
    setPrefix(capture.value.id, ev.key);
  } else {
    const binding = captureKeybinding(ev);
    if (!binding) return;
    setBinding(capture.value.id, binding);
  }
  capture.value = null;
}
function onWheel(ev: WheelEvent) {
  if (!capture.value || capture.value.prefix || !capture.value.id.startsWith("terminal.zoom")) return;
  ev.preventDefault();
  ev.stopImmediatePropagation();
  const binding = captureWheelBinding(ev);
  if (!binding) return;
  setBinding(capture.value.id, binding);
  capture.value = null;
}
function onClick(ev: MouseEvent) {
  if (!capture.value?.id.startsWith("terminal.openLink") || ev.button !== 0) return;
  ev.preventDefault(); ev.stopImmediatePropagation();
  setBinding(capture.value.id, { key: "Click", ctrl: ev.ctrlKey, shift: ev.shiftKey, alt: ev.altKey, meta: ev.metaKey });
  capture.value = null;
}
function clear(id: string, prefix = false) {
  capture.value = null;
  if (prefix) setPrefix(id, null);
  else setBinding(id, null);
}
function reset(id: string) {
  capture.value = null;
  resetBinding(id);
  resetPrefix(id);
}
onMounted(() => {
  window.addEventListener("click", onClick, true);
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("wheel", onWheel, { capture: true, passive: false });
});
onUnmounted(() => {
  window.removeEventListener("click", onClick, true);
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("wheel", onWheel, true);
});
</script>

<template>
  <div class="panel-header">
    <div>
      <h2>{{ t("Keybindings") }}</h2>
      <p>{{ t("Click a shortcut to record keys. For terminal zoom, you can also scroll the wheel with modifiers. Right-click to clear.") }}</p>
      <p>{{ t("Prefix combinations: press the prefix key, then the second key. Both can be changed below.") }} ({{ formatKeybinding(bindingFor('prefix.activate')) }})</p>
    </div>
    <button class="reset-all" @click="capture = null; resetAll()">{{ t("Reset all") }}</button>
  </div>
  <p v-if="conflicts.size" class="warn">{{ t("Conflicting shortcuts detected. Change or clear the highlighted bindings.") }}</p>
  <div class="kb-grid">
    <section v-for="group in groups" :key="group.id" class="kb-card">
      <h3>{{ t(group.label) }} <span>{{ group.actions.length }}</span></h3>
      <div v-for="a in group.actions" :key="a.id" :data-action-id="a.id" :class="['kb-row', { conflict: conflicts.has(a.id) }]">
        <span class="kb-label">{{ t(a.label) }}</span>
        <div class="binding-cells">
          <button :class="['key-cell', { capturing: capture?.id === a.id && !capture.prefix }]"
            :title="t('Click to record · Right-click to clear')" @click="start(a.id)" @contextmenu.prevent="clear(a.id)">
            {{ display(a.id) }}
          </button>
          <button v-if="!a.scope" :class="['prefix-cell', { capturing: capture?.id === a.id && capture.prefix }]"
            :title="t('Prefix second key')" @click="start(a.id, true)" @contextmenu.prevent="clear(a.id, true)">
            {{ t("Prefix") }}: {{ display(a.id, true) }}
          </button>
        </div>
        <button class="reset" :title="t('Reset to default')" :disabled="!isOverridden(a.id) && !isOverridden(`${a.id}:prefix`)" @click="reset(a.id)">↺</button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.panel-header { display: flex; gap: 20px; justify-content: space-between; align-items: flex-start; margin-bottom: 20px; }
h2 { margin: 0 0 8px; font-size: 20px; }
p { margin: 5px 0; color: #aaa; font-size: 12px; line-height: 1.5; }
.kb-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr)); gap: 16px; align-items: start; }
.kb-card { min-width: 0; background: #202020; border: 1px solid #333; border-radius: 10px; padding: 16px; }
h3 { margin: 0 0 12px; font-size: 13px; } h3 span { color: #888; margin-left: 8px; }
.kb-row { display: grid; grid-template-columns: minmax(100px, 1fr) minmax(0, 1.2fr) 26px; align-items: center; gap: 8px; padding: 9px 0; border-top: 1px solid #303030; }
.kb-label { font-size: 12px; overflow-wrap: anywhere; }
.binding-cells { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
button { color: #ddd; background: #2a2a2a; border: 1px solid #454545; border-radius: 5px; padding: 6px 8px; cursor: pointer; font-size: 11px; overflow-wrap: anywhere; }
button:hover { border-color: #888; } button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.key-cell, .prefix-cell { font-family: "Cascadia Mono", Consolas, monospace; }
.prefix-cell { color: #aaa; background: transparent; }
.capturing { border-color: var(--accent); color: var(--accent); }
.reset { padding: 4px; font-size: 16px; } .reset:disabled { opacity: .25; cursor: default; }
.reset-all { white-space: nowrap; }
.conflict .key-cell, .conflict .prefix-cell { border-color: #dba45e; }
.warn { color: #dba45e; margin-bottom: 12px; }
</style>
