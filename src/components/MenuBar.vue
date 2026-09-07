<script setup lang="ts">
import appIcon from "../../src-tauri/icons/32x32.png";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Icon } from "@iconify/vue";
import { useKeybindings } from "../composables/useKeybindings";
import { useShellPanels } from "../composables/useShellPanels";
import { dispatchAction } from "../composables/useGlobalShortcuts";
import { useSettings } from "../composables/useSettings";
import { formatKeybinding, type ActionId } from "../lib/keybindings";
import { toggleFlyoutGroup } from "../lib/menu-flyout";
import { groupMenuItems } from "../lib/menu-groups";
import {
  chevronRightIcon,
  moreHorizontalIcon,
  panelLeftIcon,
} from "../lib/offline-icons";

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
const { panels, toggleLeft } = useShellPanels();

const isOverflowMenuOpen = ref(false);
const activeGroupId = ref<string | null>(null);
const rootRef = ref<HTMLDivElement | null>(null);

const leftToggleTitle = computed(
  () => `Toggle Left Panel  (${formatKeybinding(bindingFor("view.toggleLeftPanel"))})`,
);

async function quitApp() {
  try {
    const w = getCurrentWebviewWindow();
    await w.close();
  } catch (e) {
    console.warn("quit failed", e);
  }
}

const menuGroups = computed<MenuDef[]>(() => groupMenuItems<MenuItem>([
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
]));

function toggleOverflowMenu() {
  isOverflowMenuOpen.value = !isOverflowMenuOpen.value;
  activeGroupId.value = null;
}

function showGroup(id: string) {
  activeGroupId.value = id;
}

function toggleGroup(id: string) {
  activeGroupId.value = toggleFlyoutGroup(activeGroupId.value, id);
}

function closeMenu() {
  isOverflowMenuOpen.value = false;
  activeGroupId.value = null;
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
  if (ev.key === "Escape" && isOverflowMenuOpen.value) {
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
  <div ref="rootRef" class="menubar" data-tauri-drag-region>
    <div class="app-icon" data-tauri-drag-region><img :src="appIcon" alt="winmux" data-tauri-drag-region draggable="false" /></div>
    <div class="overflow-menu">
      <button
        class="corner-toggle"
        :class="{ active: isOverflowMenuOpen }"
        title="Application menu"
        type="button"
        aria-label="Application menu"
        aria-haspopup="menu"
        :aria-expanded="isOverflowMenuOpen"
        @click.stop="toggleOverflowMenu"
      >
        <Icon class="ico" :icon="moreHorizontalIcon" />
      </button>
      <div v-if="isOverflowMenuOpen" class="dropdown root-dropdown" role="menu" @click.stop>
        <div
          v-for="group in menuGroups"
          :key="group.id"
          class="menu-group"
          @mouseenter="showGroup(group.id)"
        >
          <button
            class="group-item"
            :class="{ active: activeGroupId === group.id }"
            type="button"
            role="menuitem"
            :aria-expanded="activeGroupId === group.id"
            @click="toggleGroup(group.id)"
          >
            <span>{{ group.label }}</span>
            <Icon class="group-arrow" :icon="chevronRightIcon" />
          </button>
          <div
            v-if="activeGroupId === group.id"
            class="dropdown submenu"
            role="menu"
          >
            <template v-for="(item, i) in group.items" :key="i">
              <div v-if="item.kind === 'separator'" class="sep" />
              <div
                v-else
                class="item"
                :class="{ disabled: item.disabled }"
                role="menuitem"
                @click="!item.disabled && runItem(item)"
              >
                <span class="item-label">{{ item.label }}</span>
                <span class="item-shortcut">{{ shortcutLabel(item) }}</span>
              </div>
            </template>
          </div>
        </div>
      </div>
    </div>
    <div class="spacer" data-tauri-drag-region />
    <button
      class="corner-toggle"
      :class="{ active: panels.left.open }"
      :title="leftToggleTitle"
      :aria-label="leftToggleTitle"
      :aria-expanded="panels.left.open"
      type="button"
      @click="toggleLeft"
    >
      <Icon class="ico" :icon="panelLeftIcon" />
    </button>
  </div>
</template>

<style scoped>
.menubar {
  display: flex;
  align-items: stretch;
  height: var(--titlebar-height);
  background: #252525;
  border-bottom: 1px solid #111;
  user-select: none;
  font-size: 12px;
  flex-shrink: 0;
  z-index: 50;
}
.app-icon { width: 34px; flex-shrink: 0; display: flex; align-items: center; justify-content: center; }
.app-icon img { width: 24px; height: 24px; }
.corner-toggle:focus-visible { outline: 1px solid #4ec9b0; outline-offset: -2px; }
.overflow-menu {
  position: relative;
  display: flex;
  height: 100%;
}
.spacer { flex: 1; }
.corner-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 100%;
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
.root-dropdown {
  top: 0;
  left: 100%;
  min-width: 160px;
}
.menu-group {
  position: relative;
}
.group-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  height: 28px;
  padding: 0 8px 0 12px;
  border: 0;
  background: transparent;
  color: #d4d4d4;
  cursor: pointer;
  font: inherit;
  text-align: left;
}
.group-item:hover,
.group-item.active {
  background: #2e2e2e;
}
.group-arrow {
  color: #8f8f8f;
  font-size: 15px;
}
.submenu {
  top: -4px;
  left: calc(100% + 4px);
  min-width: 240px;
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
