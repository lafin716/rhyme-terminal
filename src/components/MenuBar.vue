<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Icon } from "@iconify/vue";
import { useKeybindings } from "../composables/useKeybindings";
import { useShellPanels } from "../composables/useShellPanels";
import { dispatchAction } from "../composables/useGlobalShortcuts";
import { useSettings } from "../composables/useSettings";
import { useFireworks } from "../composables/useFireworks";
import { formatKeybinding, type ActionId } from "../lib/keybindings";
import { panelLeftIcon, panelRightIcon, fireworksIcon } from "../lib/offline-icons";

type MenuItem =
  | { kind: "action"; label: string; actionId: ActionId; disabled?: boolean }
  | { kind: "cmd"; label: string; run: () => void; shortcut?: string; disabled?: boolean }
  | { kind: "separator" };

interface MenuDef {
  id: string;
  label: string;
  items: MenuItem[];
}

const { bindingFor, prefixFor } = useKeybindings();
const { openSettings } = useSettings();
const { panels, toggleLeft, toggleRight } = useShellPanels();
const { launch: launchFireworks } = useFireworks();

const openMenu = ref<string | null>(null);
const rootRef = ref<HTMLDivElement | null>(null);

const leftToggleTitle = computed(
  () => `Toggle Left Panel  (${formatKeybinding(bindingFor("view.toggleLeftPanel"))})`,
);
const rightToggleTitle = computed(
  () => `Toggle Right Panel  (${formatKeybinding(bindingFor("view.toggleRightPanel"))})`,
);

async function quitApp() {
  try {
    const w = getCurrentWebviewWindow();
    await w.close();
  } catch (e) {
    console.warn("quit failed", e);
  }
}

const menus = computed<MenuDef[]>(() => [
  {
    id: "file",
    label: "File",
    items: [
      { kind: "action", label: "New Terminal", actionId: "session.new" },
      { kind: "separator" },
      { kind: "cmd", label: "Quit", run: quitApp },
    ],
  },
  {
    id: "edit",
    label: "Edit",
    items: [
      { kind: "action", label: "Rename Session", actionId: "session.rename" },
    ],
  },
  {
    id: "view",
    label: "View",
    items: [
      { kind: "cmd", label: "Reload", run: () => location.reload() },
    ],
  },
  {
    id: "terminal",
    label: "Terminal",
    items: [
      { kind: "action", label: "New Terminal", actionId: "session.new" },
      { kind: "action", label: "Kill Focused", actionId: "session.kill" },
      { kind: "separator" },
      { kind: "action", label: "Split Horizontally", actionId: "pane.splitHorizontal" },
      { kind: "action", label: "Split Vertically", actionId: "pane.splitVertical" },
      { kind: "separator" },
      { kind: "action", label: "Next Tab", actionId: "session.cycleNext" },
      { kind: "action", label: "Previous Tab", actionId: "session.cyclePrev" },
    ],
  },
  {
    id: "settings",
    label: "Settings",
    items: [
      { kind: "action", label: "Preferences...", actionId: "settings.open" },
    ],
  },
  {
    id: "help",
    label: "Help",
    items: [
      { kind: "cmd", label: "About winmux", run: () => alert("winmux — terminal multiplexer") },
    ],
  },
]);

function toggleMenu(id: string) {
  openMenu.value = openMenu.value === id ? null : id;
}

function hoverMenu(id: string) {
  if (openMenu.value !== null) openMenu.value = id;
}

function closeMenu() {
  openMenu.value = null;
}

function runItem(item: MenuItem) {
  if (item.kind === "separator") return;
  closeMenu();
  if (item.kind === "action") {
    if (item.actionId === "settings.open") openSettings();
    else dispatchAction(item.actionId);
  } else {
    item.run();
  }
}

function shortcutLabel(item: MenuItem): string {
  if (item.kind === "action") {
    return formatKeybinding(bindingFor(item.actionId), prefixFor(item.actionId));
  }
  if (item.kind === "cmd" && item.shortcut) return item.shortcut;
  return "";
}

function onDocClick(ev: MouseEvent) {
  if (!rootRef.value) return;
  if (!rootRef.value.contains(ev.target as Node)) closeMenu();
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === "Escape" && openMenu.value) {
    ev.stopPropagation();
    closeMenu();
  }
}

onMounted(() => {
  window.addEventListener("click", onDocClick);
  window.addEventListener("keydown", onKey);
});
onUnmounted(() => {
  window.removeEventListener("click", onDocClick);
  window.removeEventListener("keydown", onKey);
});
</script>

<template>
  <div ref="rootRef" class="menubar">
    <button
      class="corner-toggle"
      :class="{ active: panels.left.open }"
      :title="leftToggleTitle"
      type="button"
      @click="toggleLeft"
    >
      <Icon class="ico" :icon="panelLeftIcon" />
    </button>
    <div
      v-for="m in menus"
      :key="m.id"
      class="menu"
      :class="{ active: openMenu === m.id }"
      @click.stop="toggleMenu(m.id)"
      @mouseenter="hoverMenu(m.id)"
    >
      <span class="label">{{ m.label }}</span>
      <div v-if="openMenu === m.id" class="dropdown" @click.stop>
        <template v-for="(item, i) in m.items" :key="i">
          <div v-if="item.kind === 'separator'" class="sep" />
          <div
            v-else
            class="item"
            :class="{ disabled: item.disabled }"
            @click="!item.disabled && runItem(item)"
          >
            <span class="item-label">{{ item.label }}</span>
            <span class="item-shortcut">{{ shortcutLabel(item) }}</span>
          </div>
        </template>
      </div>
    </div>
    <div class="spacer" />
    <button
      class="corner-toggle fireworks-btn"
      title="Fireworks!"
      type="button"
      @click="launchFireworks"
    >
      <Icon class="ico" :icon="fireworksIcon" />
    </button>
    <button
      class="corner-toggle"
      :class="{ active: panels.right.open }"
      :title="rightToggleTitle"
      type="button"
      @click="toggleRight"
    >
      <Icon class="ico" :icon="panelRightIcon" />
    </button>
  </div>
</template>

<style scoped>
.menubar {
  display: flex;
  align-items: stretch;
  height: 28px;
  background: #252525;
  border-bottom: 1px solid #111;
  user-select: none;
  font-size: 12px;
  flex-shrink: 0;
  position: relative;
  z-index: 50;
}
.menu {
  position: relative;
  display: flex;
  align-items: center;
  padding: 0 10px;
  color: #d4d4d4;
  cursor: pointer;
}
.menu:hover, .menu.active {
  background: #3a3a3a;
}
.label { line-height: 1; }
.spacer { flex: 1; }
.corner-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  border: none;
  background: transparent;
  color: #9a9a9a;
  cursor: pointer;
  flex-shrink: 0;
}
.corner-toggle:hover {
  background: #3a3a3a;
  color: #d4d4d4;
}
.corner-toggle.active { color: #4ec9b0; }
/* Momentary press feedback for the (stateless) fireworks trigger. */
.fireworks-btn:active { color: #4ec9b0; }
.corner-toggle .ico { font-size: 15px; }
.dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  min-width: 240px;
  background: #252525;
  border: 1px solid #111;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.55);
  padding: 4px 0;
  z-index: 100;
}
.item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 12px;
  color: #d4d4d4;
  cursor: pointer;
  gap: 24px;
}
.item:hover { background: #2e2e2e; }
.item.disabled { color: #555; cursor: not-allowed; }
.item.disabled:hover { background: transparent; }
.item-shortcut {
  color: #888;
  font-size: 11px;
  font-family: Consolas, "Cascadia Mono", monospace;
}
.sep {
  height: 1px;
  background: #111;
  margin: 4px 0;
}
</style>
