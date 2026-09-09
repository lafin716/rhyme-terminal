<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { Icon } from "@iconify/vue";
import { t } from "../composables/useI18n";
import { useSettings } from "../composables/useSettings";
import { formatKeybinding } from "../lib/keybindings";
import { useKeybindings } from "../composables/useKeybindings";
const props = defineProps<{ target: string; x: number; y: number; web: boolean }>();
const emit = defineEmits<{ close: []; open: [external: boolean] }>();
const panel = ref<HTMLElement>();
const left = ref(props.x);
const top = ref(props.y);
const copied = ref(false);
const error = ref("");
const { bindingFor } = useKeybindings();
const { openSettings } = useSettings();
async function copy() {
  try { await navigator.clipboard.writeText(props.target); copied.value = true; }
  catch (e) { error.value = String(e); }
}
function outside(e: PointerEvent) {
  if (!panel.value?.contains(e.target as Node)) emit("close");
}
function key(e: KeyboardEvent) {
  if (e.key === "Escape") { e.preventDefault(); e.stopImmediatePropagation(); emit("close"); }
  if (e.key === "Tab" || e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault(); e.stopImmediatePropagation();
    const buttons = Array.from(panel.value?.querySelectorAll<HTMLButtonElement>("button") ?? []);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const step = e.shiftKey || e.key === "ArrowUp" ? -1 : 1;
    buttons[(index + step + buttons.length) % buttons.length]?.focus();
  }
}
function settings() { emit("close"); openSettings(); }
onMounted(async () => {
  await nextTick();
  const rect = panel.value!.getBoundingClientRect();
  left.value = Math.max(8, Math.min(props.x, window.innerWidth - rect.width - 8));
  top.value = Math.max(8, props.y + rect.height + 8 <= window.innerHeight ? props.y + 8 : props.y - rect.height - 8);
  panel.value?.querySelector<HTMLButtonElement>(".action")?.focus();
  window.addEventListener("pointerdown", outside, true);
  window.addEventListener("keydown", key, true);
  window.addEventListener("resize", close);
});
function close() { emit("close"); }
onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", outside, true);
  window.removeEventListener("keydown", key, true);
  window.removeEventListener("resize", close);
});
</script>

<template>
  <Teleport to="body">
    <div ref="panel" class="link-menu" role="dialog" :aria-label="t('Open resource')" :style="{ left: `${left}px`, top: `${top}px` }" @pointerdown.stop @click.stop>
      <div class="heading">
        <span class="target" :title="target">{{ target }}</span>
        <button :title="t(copied ? 'Copied' : 'Copy')" :aria-label="t(copied ? 'Copied' : 'Copy')" @click="copy"><Icon :icon="copied ? 'lucide:check' : 'lucide:copy'" /></button>
        <button :title="t('Settings')" :aria-label="t('Settings')" @click="settings"><Icon icon="lucide:settings" /></button>
      </div>
      <button class="action" @click="emit('open', false)">
        <Icon :icon="web ? 'lucide:globe' : 'lucide:file-text'" />
        <span>{{ t(web ? 'rhyme Browser' : 'Open in Editor') }}</span>
        <kbd v-if="bindingFor('terminal.openLink')">{{ formatKeybinding(bindingFor('terminal.openLink')) }}</kbd>
      </button>
      <button class="action" @click="emit('open', true)">
        <Icon :icon="web ? 'lucide:external-link' : 'lucide:folder-open'" />
        <span>{{ t(web ? 'System Browser' : 'Show in Explorer') }}</span>
        <kbd v-if="bindingFor('terminal.openLinkExternal')">{{ formatKeybinding(bindingFor('terminal.openLinkExternal')) }}</kbd>
      </button>
      <p v-if="error" role="alert">{{ error }}</p>
    </div>
  </Teleport>
</template>

<style scoped>
.link-menu { position: fixed; z-index: 10020; width: 334px; max-width: calc(100vw - 16px); padding: 4px; background: #141414; color: #e5e5e5; border: 1px solid #414141; border-radius: 7px; box-shadow: 0 8px 24px #0006; font-size: 12px; }
.heading { display: flex; align-items: center; gap: 4px; padding: 4px 5px 8px; margin-bottom: 4px; border-bottom: 1px solid #ffffff12; }
.target { flex: 1; min-width: 0; max-height: 64px; overflow: auto; overflow-wrap: anywhere; color: #aaa; font: 12px/1.4 Consolas, monospace; user-select: text; }
button { color: inherit; background: none; border: 0; border-radius: 4px; cursor: pointer; }
.heading button { display: grid; place-items: center; flex-shrink: 0; width: 25px; height: 25px; color: #aaa; }
.action { display: flex; align-items: center; gap: 8px; width: 100%; padding: 8px 6px; text-align: left; font-size: 12px; }
.action span { flex: 1; }
svg { width: 15px; height: 15px; flex-shrink: 0; }
button:hover, button:focus-visible { background: #ffffff12; outline: none; }
button:focus-visible { box-shadow: inset 0 0 0 1px #888; }
kbd { padding: 2px 4px; background: #262626; border: 1px solid #333; border-radius: 4px; color: #aaa; font: 10px Consolas, monospace; }
p { color: #f99; padding: 4px; overflow-wrap: anywhere; }
</style>
