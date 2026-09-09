<script setup lang="ts">
import { t } from "../composables/useI18n";
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { Icon, type IconifyIcon } from "@iconify/vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { usePalette, closePalette } from "../composables/usePalette";
import { usePrefs } from "../composables/usePrefs";
import { folderOpenIcon, terminalIcon } from "../lib/offline-icons";
import { api, stringToBase64 } from "../lib/tauri";

const { state, items } = usePalette();
const { prefs } = usePrefs();

const RADIUS = 80;
const ITEM_SIZE = 56;
const VIEWPORT_MARGIN = 8;

interface PaletteAction {
  id: string;
  label: string;
  icon: IconifyIcon;
  builtin: boolean;
}

const allItems = computed<PaletteAction[]>(() => [
  { id: "__open_folder", label: t("Open Folder"), icon: folderOpenIcon, builtin: true },
  ...items.map((item) => ({
    id: item.id,
    label: item.label || t("(unnamed)"),
    icon: terminalIcon,
    builtin: false,
  })),
]);

const rootEl = ref<HTMLElement | null>(null);
const menuPosition = ref({ left: 0, top: 0 });
const activeIndex = ref(0);
const previousFocus = ref<HTMLElement | null>(null);

function offsetFor(i: number, n: number): { left: string; top: string } {
  const angle = -Math.PI / 2 + (2 * Math.PI * i) / Math.max(1, n);
  const cx = Math.cos(angle) * RADIUS;
  const cy = Math.sin(angle) * RADIUS;
  return {
    left: `${cx - ITEM_SIZE / 2}px`,
    top: `${cy - ITEM_SIZE / 2}px`,
  };
}

async function activate(item: PaletteAction) {
  const sid = state.sessionId;
  closePalette();
  if (!sid) return;

  if (item.builtin && item.id === "__open_folder") {
    let selected: string | string[] | null;
    try {
      selected = await openDialog({ directory: true, multiple: false });
    } catch (e) {
      console.warn("dialog open failed", e);
      return;
    }
    if (typeof selected !== "string") return;
    try {
      await api.writeSession(sid, stringToBase64(`cd "${selected}"\r`));
    } catch (e) {
      console.warn("writeSession failed", e);
    }
    return;
  }

  const definition = items.find((itemDefinition) => itemDefinition.id === item.id);
  if (!definition) return;
  const text = definition.command + (definition.autoRun ? "\r" : "");
  if (!text) return;
  try {
    await api.writeSession(sid, stringToBase64(text));
  } catch (e) {
    console.warn("writeSession failed", e);
  }
}

function positionContextMenu() {
  const menu = rootEl.value;
  if (!menu) return;
  const rect = menu.getBoundingClientRect();
  menuPosition.value = {
    left: Math.min(
      Math.max(VIEWPORT_MARGIN, state.x),
      Math.max(VIEWPORT_MARGIN, window.innerWidth - rect.width - VIEWPORT_MARGIN),
    ),
    top: Math.min(
      Math.max(VIEWPORT_MARGIN, state.y),
      Math.max(VIEWPORT_MARGIN, window.innerHeight - rect.height - VIEWPORT_MARGIN),
    ),
  };
}

function focusActiveItem() {
  if (prefs.paletteUiMode !== "context") return;
  const buttons = rootEl.value?.querySelectorAll<HTMLButtonElement>(".context-item");
  buttons?.[activeIndex.value]?.focus();
}

function moveActive(delta: number) {
  const count = allItems.value.length;
  if (count === 0) return;
  activeIndex.value = (activeIndex.value + delta + count) % count;
  focusActiveItem();
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === "Escape") {
    ev.preventDefault();
    ev.stopPropagation();
    closePalette();
    return;
  }
  if (prefs.paletteUiMode !== "context") return;

  if (ev.key === "ArrowDown") {
    ev.preventDefault();
    ev.stopPropagation();
    moveActive(1);
  } else if (ev.key === "ArrowUp") {
    ev.preventDefault();
    ev.stopPropagation();
    moveActive(-1);
  } else if (ev.key === "Home") {
    ev.preventDefault();
    ev.stopPropagation();
    activeIndex.value = 0;
    focusActiveItem();
  } else if (ev.key === "End") {
    ev.preventDefault();
    ev.stopPropagation();
    activeIndex.value = Math.max(0, allItems.value.length - 1);
    focusActiveItem();
  } else if (ev.key === "Enter" || ev.key === " ") {
    ev.preventDefault();
    ev.stopPropagation();
    const item = allItems.value[activeIndex.value];
    if (item) void activate(item);
  }
}

function onOutsidePointer(ev: PointerEvent) {
  if (rootEl.value && !rootEl.value.contains(ev.target as Node)) {
    closePalette();
  }
}

function onContextItemFocus(index: number) {
  activeIndex.value = index;
}

watch(
  () => state.open,
  async (isOpen) => {
    if (isOpen) {
      previousFocus.value = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
      activeIndex.value = 0;
      window.addEventListener("keydown", onKey, true);
      window.addEventListener("pointerdown", onOutsidePointer, true);
      if (prefs.paletteUiMode === "context") {
        menuPosition.value = { left: state.x, top: state.y };
        await nextTick();
        positionContextMenu();
        focusActiveItem();
      }
    } else {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onOutsidePointer, true);
      previousFocus.value?.focus();
      previousFocus.value = null;
    }
  },
);

watch(
  () => prefs.paletteUiMode,
  async () => {
    if (!state.open || prefs.paletteUiMode !== "context") return;
    activeIndex.value = 0;
    menuPosition.value = { left: state.x, top: state.y };
    await nextTick();
    positionContextMenu();
    focusActiveItem();
  },
);

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("pointerdown", onOutsidePointer, true);
});
</script>

<template>
  <div
    v-if="state.open && prefs.paletteUiMode === 'context'"
    ref="rootEl"
    class="context-menu"
    :style="{ left: menuPosition.left + 'px', top: menuPosition.top + 'px' }"
    role="menu"
    :aria-label="t('Terminal palette')"
  >
    <template v-for="(item, index) in allItems" :key="item.id">
      <div v-if="index === 1" class="context-separator" role="separator" />
      <button
        class="context-item"
        :class="{ active: activeIndex === index }"
        type="button"
        role="menuitem"
        @focus="onContextItemFocus(index)"
        @mouseenter="activeIndex = index"
        @click.stop="activate(item)"
      >
        <Icon class="context-icon" :icon="item.icon" />
        <span class="context-label">{{ item.label }}</span>
      </button>
    </template>
  </div>

  <div
    v-else-if="state.open"
    ref="rootEl"
    class="radial-root"
    :style="{ left: state.x + 'px', top: state.y + 'px' }"
  >
    <div class="center-dot" />
    <button
      v-for="(item, i) in allItems"
      :key="item.id"
      class="radial-item"
      :style="offsetFor(i, allItems.length)"
      :title="item.label"
      @click.stop="activate(item)"
    >
      <Icon class="ico" :icon="item.icon" />
      <span class="lbl">{{ item.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.context-menu {
  position: fixed;
  z-index: 200;
  width: 224px;
  max-height: calc(100vh - 16px);
  overflow-y: auto;
  padding: 4px;
  background: #252525;
  border: 1px solid #111;
  border-radius: 4px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.58);
}
.context-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 30px;
  padding: 0 9px;
  color: #d4d4d4;
  text-align: left;
  background: transparent;
  border: 0;
  border-radius: 2px;
  cursor: pointer;
}
.context-item:hover,
.context-item.active,
.context-item:focus-visible {
  color: #fff;
  background: #094771;
  outline: none;
}
.context-icon {
  flex: 0 0 16px;
  color: #4ec9b0;
  font-size: 16px;
}
.context-label {
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.context-separator {
  height: 1px;
  margin: 4px 6px;
  background: #111;
}

.radial-root {
  position: fixed;
  z-index: 200;
  width: 0;
  height: 0;
  pointer-events: none;
}
.radial-root > * {
  pointer-events: auto;
}
.center-dot {
  position: absolute;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #4ec9b0;
  left: -5px;
  top: -5px;
  opacity: 0.7;
  pointer-events: none;
}
.radial-item {
  position: absolute;
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: #252525;
  border: 1px solid #333;
  color: #e6e6e6;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  cursor: pointer;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  transition: background 120ms ease, transform 120ms ease, border-color 120ms ease;
}
.radial-item:hover {
  background: #2e2e2e;
  border-color: #4ec9b0;
  transform: scale(1.05);
}
.radial-item .ico {
  font-size: 20px;
  color: #4ec9b0;
}
.radial-item .lbl {
  font-size: 9px;
  color: #d4d4d4;
  max-width: 52px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: center;
}
</style>
