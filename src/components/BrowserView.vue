<script setup lang="ts">
import { t } from "../composables/useI18n";
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Webview } from "@tauri-apps/api/webview";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/tauri";
import type { BrowserTab } from "../composables/useResources";
import { useResources } from "../composables/useResources";
import { useSettings } from "../composables/useSettings";
import { confirmState } from "../composables/useConfirm";
import { useDragState } from "../composables/useDragState";

const props = defineProps<{ tab: BrowserTab; active: boolean }>();

const viewport = ref<HTMLDivElement | null>(null);
const address = ref(props.tab.url);
const error = ref("");
const { settingsOpen } = useSettings();
const drag = useDragState();
let child: Webview | null = null;
let observer: ResizeObserver | null = null;
let disposed = false;

async function syncBounds() {
  if (!child || !viewport.value || !props.active) return;
  const rect = viewport.value.getBoundingClientRect();
  if (rect.width < 1 || rect.height < 1) return;
  await Promise.all([
    child.setPosition(new LogicalPosition(rect.left, rect.top)),
    child.setSize(new LogicalSize(rect.width, rect.height)),
  ]).catch((e) => {
    error.value = String(e);
  });
}

async function createChild() {
  await nextTick();
  if (!viewport.value || disposed) return;
  const rect = viewport.value.getBoundingClientRect();
  const existing = await Webview.getByLabel(props.tab.webviewLabel);
  if (existing) {
    await existing.close().catch(() => {});
    await new Promise((resolve) => window.setTimeout(resolve, 25));
  }
  child = new Webview(getCurrentWindow(), props.tab.webviewLabel, {
    url: props.tab.url,
    x: rect.left,
    y: rect.top,
    width: Math.max(1, rect.width),
    height: Math.max(1, rect.height),
    focus: props.active,
    dragDropEnabled: false,
  });
  child.once("tauri://error", (event) => {
    error.value = String(event.payload);
  });
  if (props.active) {
    await syncBounds();
    await child.show().catch(() => {});
  } else {
    await child.hide().catch(() => {});
  }
}

async function navigate() {
  try {
    const url = new URL(address.value.trim());
    if (!["http:", "https:"].includes(url.protocol)) throw new Error(t("Only HTTP(S) URLs are allowed."));
    address.value = url.toString();
    useResources().updateBrowserUrl(props.tab.id, address.value);
    await api.browserNavigate(props.tab.webviewLabel, address.value);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}

watch(
  [() => props.active, settingsOpen, () => confirmState.open, () => drag.state.active],
  async ([active, settings, confirming, dragging]) => {
    if (!child) return;
    if (active && !settings && !confirming && !dragging) {
      await nextTick();
      await syncBounds();
      await child.show().catch(() => {});
    } else {
      await child.hide().catch(() => {});
    }
  },
);

onMounted(() => {
  createChild().catch((e) => {
    error.value = String(e);
  });
  observer = new ResizeObserver(() => {
    syncBounds().catch(() => {});
  });
  if (viewport.value) observer.observe(viewport.value);
  window.addEventListener("resize", syncBounds);
});

onBeforeUnmount(() => {
  disposed = true;
  observer?.disconnect();
  observer = null;
  window.removeEventListener("resize", syncBounds);
  child?.close().catch(() => {});
  child = null;
});
</script>

<template>
  <div class="browser-view" :class="{ hidden: !active }">
    <div class="toolbar">
      <button :title="t('Back')" @click="api.browserBack(tab.webviewLabel)">‹</button>
      <button :title="t('Forward')" @click="api.browserForward(tab.webviewLabel)">›</button>
      <button :title="t('Reload')" @click="api.browserReload(tab.webviewLabel)">↻</button>
      <input v-model="address" @keydown.enter="navigate" />
      <button :title="t('Open in default browser')" @click="openUrl(address)">↗</button>
    </div>
    <div ref="viewport" class="viewport">
      <span v-if="error" class="error">{{ error }}</span>
    </div>
  </div>
</template>

<style scoped>
.browser-view {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: #1e1e1e;
}
.browser-view.hidden { visibility: hidden; pointer-events: none; }
.toolbar {
  height: 34px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 3px 6px;
  background: #252525;
  border-bottom: 1px solid #111;
}
button {
  width: 27px;
  height: 26px;
  border: 0;
  background: transparent;
  color: #ccc;
  cursor: pointer;
  font-size: 16px;
}
button:hover { background: #3a3a3a; }
input {
  min-width: 0;
  flex: 1;
  height: 25px;
  border: 1px solid #444;
  background: #1e1e1e;
  color: #ddd;
  padding: 0 8px;
}
.viewport { position: relative; flex: 1; min-height: 0; }
.error { position: absolute; inset: 20px; color: #f48771; font-size: 12px; }
</style>
