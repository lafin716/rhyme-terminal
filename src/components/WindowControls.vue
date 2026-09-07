<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { Icon } from "@iconify/vue";
import { panelRightIcon } from "../lib/offline-icons";
import { useShellPanels } from "../composables/useShellPanels";

const { panels, toggleRight } = useShellPanels();
const maximized = ref(false);
let unlisten: UnlistenFn | undefined;
let disposed = false;
async function run(action: "minimize" | "toggleMaximize" | "close") {
  try { await getCurrentWindow()[action](); }
  catch (error) { console.warn(`Window ${action} failed`, error); }
}
onMounted(async () => {
  if (!isTauri()) return;
  const win = getCurrentWindow();
  const update = async () => { maximized.value = await win.isMaximized(); };
  try {
    unlisten = await win.onResized(() => { void update().catch(console.warn); });
    if (disposed) { unlisten(); unlisten = undefined; return; }
    await update();
  } catch (error) { console.warn("Window state listener failed", error); }
});
onBeforeUnmount(() => { disposed = true; unlisten?.(); });
</script>

<template>
  <div class="window-controls">
    <button class="panel-toggle" :class="{ active: panels.right.open }" :title="'Toggle Right Panel (Explorer)'" :aria-label="'Toggle Right Panel (Explorer)'" @click="toggleRight">
      <Icon :icon="panelRightIcon" />
    </button>
    <button :title="'Minimize'" :aria-label="'Minimize'" @click="run('minimize')">
      <svg viewBox="0 0 12 12"><path d="M1 6.5h10" /></svg>
    </button>
    <button :title="maximized ? 'Restore' : 'Maximize'" :aria-label="maximized ? 'Restore' : 'Maximize'" @click="run('toggleMaximize')">
      <svg viewBox="0 0 12 12"><path v-if="maximized" d="M3.5 3.5v-2h7v7h-2m-7-5h7v7h-7z" /><path v-else d="M1.5 1.5h9v9h-9z" /></svg>
    </button>
    <button class="close-window" :title="'Close'" :aria-label="'Close'" @click="run('close')">
      <svg viewBox="0 0 12 12"><path d="m1.5 1.5 9 9m0-9-9 9" /></svg>
    </button>
  </div>
</template>

<style scoped>
.window-controls { position: fixed; top: 0; right: 0; height: var(--titlebar-height); width: 172px; display: flex; background: #252525; z-index: 60; }
button { width: 46px; height: 100%; flex-shrink: 0; display: flex; align-items: center; justify-content: center; border: 0; background: transparent; color: #aaa; cursor: pointer; }
button:hover { background: #3a3a3a; color: #fff; }
button:focus-visible { outline: 1px solid #4ec9b0; outline-offset: -2px; }
.panel-toggle { width: 34px; font-size: 15px; }
.panel-toggle.active { color: #4ec9b0; }
.close-window:hover { background: #c42b1c; color: #fff; }
svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1; }
</style>
