<script setup lang="ts">
import { t } from "../composables/useI18n";
import { useTitlebarInsets } from "../composables/useTitlebarInsets";
import {
  ref,
  computed,
  defineAsyncComponent,
  nextTick,
  onMounted,
  onUnmounted,
} from "vue";
import { Icon } from "@iconify/vue";
import SessionProfileTag from "./SessionProfileTag.vue";
import { CLI_AGENTS, profilesForAgent, resolveProfileEnv, useAccountProfiles, type CliAgentKind, type AccountProfile } from "../composables/useAccountProfiles";
import { resolveDefaultProfile } from "../lib/default-profile";
import type { LeafNode } from "../lib/layout-types";
import TerminalView from "./Terminal.vue";
import BrowserView from "./BrowserView.vue";
import { useSessions, displayName } from "../composables/useSessions";
import {
  useResources,
  type BrowserTab,
  type FileTab,
} from "../composables/useResources";
import { useWorkspaces } from "../composables/useWorkspaces";
import { useFocus } from "../composables/useFocus";
import { useDragState } from "../composables/useDragState";
import { useConfirm } from "../composables/useConfirm";
import { usePrefs } from "../composables/usePrefs";
import { sessionAgentIcon } from "../lib/session-agent-icon";
import { sessionIndicatorClass } from "../lib/session-indicator";
import {
  TERMINAL_PRESETS,
  cloneTerminalConfig,
  type TerminalConfig,
} from "../lib/terminal-config";
import {
  computeDropZone,
  leafCount,
  MAX_PANES,
  moveTabToLeaf,
  moveTabAsSplit,
  moveTabToIndex,
  zoneToSplit,
} from "../composables/useLayout";

const props = defineProps<{ leaf: LeafNode }>();
const titlebarRow = ref<HTMLElement | null>(null);
const titlebarStyle = useTitlebarInsets(titlebarRow);
const FileViewer = defineAsyncComponent(() => import("./FileViewer.vue"));

const {
  kill,
  rename,
  create,
  getById: getSessionById,
  sessionAgentStatus,
} = useSessions();
const resources = useResources();
const { activeWorkspace, replaceLayout } = useWorkspaces();
const { focusedLeafId, setFocusedLeaf } = useFocus();
const drag = useDragState();
const { confirm } = useConfirm();
const { prefs } = usePrefs();

const tabInsertIndex = ref<number | null>(null);
const editingId = ref<string | null>(null);
const editValue = ref("");
const catcherRef = ref<HTMLDivElement | null>(null);
const terminalMenuOpen = ref(false);
const profileMenuAgent = ref<CliAgentKind | null>(null);
const profileMenuRef = ref<HTMLDivElement | null>(null);
const profileMenuPosition = ref({ left: 0, top: 0 });
let profileMenuTrigger: HTMLElement | null = null;

async function openProfileMenu(agent: CliAgentKind, event: Event, focus = false) {
  profileMenuAgent.value = agent;
  profileMenuTrigger = event.currentTarget as HTMLElement;
  const rect = profileMenuTrigger.getBoundingClientRect();
  await nextTick();
  const menu = profileMenuRef.value;
  if (!menu) return;
  const width = menu.offsetWidth;
  profileMenuPosition.value = {
    left: Math.max(8, rect.right + width + 8 <= window.innerWidth ? rect.right : rect.left - width),
    top: Math.max(8, Math.min(rect.top, window.innerHeight - menu.offsetHeight - 8)),
  };
  if (focus) menu.querySelector<HTMLButtonElement>('button')?.focus();
}

function closeProfileMenu() {
  profileMenuAgent.value = null;
  profileMenuTrigger?.focus();
}

async function createWithAgent(agent: CliAgentKind, profile?: AccountProfile | null) {
  const selected = profile === undefined
    ? resolveDefaultProfile(agent, useAccountProfiles().profiles, prefs.defaultProfileId)
    : profile;
  closeTerminalMenu();
  setFocusedLeaf(props.leaf.id);
  try {
    const env = selected ? await resolveProfileEnv(selected) : undefined;
    await create({ env, launchCommand: agent });
  } catch (error) {
    alert(t("Failed to create terminal: {message}", { message: String(error) }));
  }
}
const terminalMenuButtonRef = ref<HTMLButtonElement | null>(null);
const terminalMenuRef = ref<HTMLDivElement | null>(null);
const terminalMenuPosition = ref({ left: 0, top: 0 });
const tabMenuFor = ref<string | null>(null);
const tabMenuRef = ref<HTMLDivElement | null>(null);
const tabMenuPosition = ref({ left: 0, top: 0 });

const isFocused = computed(() => focusedLeafId.value === props.leaf.id);
const effectiveTerminal = computed<TerminalConfig>(() =>
  cloneTerminalConfig(activeWorkspace.value?.settings?.terminal ?? prefs.defaultTerminal),
);
const selectableTerminals = computed(() => {
  const defaultTerminal = effectiveTerminal.value;
  return TERMINAL_PRESETS
    .filter((preset) => preset.id !== "custom")
    .map((preset) => ({
      ...preset,
      preset: preset.id,
      args: [...preset.args],
    }))
    .filter((preset) => !sameTerminalConfig(defaultTerminal, preset));
});

function sameTerminalConfig(a: TerminalConfig, b: TerminalConfig): boolean {
  return a.preset === b.preset
    && a.program.trim().toLowerCase() === b.program.trim().toLowerCase()
    && a.args.length === b.args.length
    && a.args.every((arg, index) => arg === b.args[index]);
}

function terminalLabel(terminal: TerminalConfig): string {
  return t(TERMINAL_PRESETS.find((preset) => preset.id === terminal.preset)?.label
    ?? "Custom terminal");
}

function terminalGlyph(terminal: TerminalConfig): string {
  switch (terminal.preset) {
    case "windows-powershell": return "PS";
    case "powershell": return "P7";
    case "cmd": return "C:\\";
    case "wsl": return "WSL";
    case "git-bash": return "GB";
    case "custom": return ">_";
  }
}

function sessionName(id: string): string {
  const resource = resources.getById(id);
  if (resource?.kind === "file") return resource.preview.name;
  if (resource?.kind === "browser") {
    try {
      return new URL(resource.url).hostname;
    } catch {
      return resource.url;
    }
  }
  const s = getSessionById(id);
  return s ? displayName(s.name) : id.slice(0, 6);
}

function tabKind(id: string): "terminal" | "file" | "browser" {
  return resources.getById(id)?.kind ?? "terminal";
}

function tabIcon(id: string): string {
  switch (tabKind(id)) {
    case "file": return "▤";
    case "browser": return "◎";
    default: return "›_";
  }
}

function terminalTabIcon(id: string) {
  return sessionAgentIcon(getSessionById(id)?.agent ?? "terminal");
}

function fileTab(id: string | null): FileTab | null {
  if (!id) return null;
  const tab = resources.getById(id);
  return tab?.kind === "file" ? tab : null;
}

function isFileDirty(id: string): boolean {
  const tab = resources.getById(id);
  return tab?.kind === "file" && !!tab.dirty;
}

function browserTab(id: string): BrowserTab | null {
  const tab = resources.getById(id);
  return tab?.kind === "browser" ? tab : null;
}

function selectTab(id: string) {
  props.leaf.activeTabId = id;
  setFocusedLeaf(props.leaf.id);
  closeTabContextMenu();
}

function startRename(id: string) {
  if (tabKind(id) !== "terminal") return;
  closeTabContextMenu();
  editingId.value = id;
  editValue.value = sessionName(id);
}

async function commitRename() {
  if (editingId.value && editValue.value.trim()) {
    await rename(editingId.value, editValue.value.trim());
  }
  editingId.value = null;
}

async function closeTab(id: string, ev: MouseEvent) {
  ev.stopPropagation();
  await closeTabs([id]);
}

async function closeTabs(ids: string[]) {
  closeTabContextMenu();
  const uniqueIds = [...new Set(ids)].filter((id) => props.leaf.tabs.includes(id));
  if (!uniqueIds.length) return;
  const terminalIds = uniqueIds.filter((id) => !resources.getById(id));
  if (terminalIds.length > 0) {
    const ok = await confirm({
      message: terminalIds.length === 1
        ? t('Kill session "{name}"?', { name: sessionName(terminalIds[0]) })
        : t("Kill {count} terminal sessions?", { count: terminalIds.length }),
      confirmLabel: t("Kill"),
      rememberKey: "skipKillSessionConfirm",
    });
    if (!ok) return;
  }
  for (const id of uniqueIds) {
    await closeTabById(id);
  }
}

async function closeTabById(id: string) {
  if (resources.getById(id)) {
    resources.closeResource(id);
    return;
  }
  await kill(id);
}

function onTabMouseDown(ev: MouseEvent) {
  if (ev.button !== 1) return;
  ev.preventDefault();
  ev.stopPropagation();
}

async function onTabAuxClick(ev: MouseEvent, id: string) {
  if (ev.button !== 1) return;
  ev.preventDefault();
  ev.stopPropagation();
  await closeTabs([id]);
}

async function openTabContextMenu(ev: MouseEvent, id: string) {
  ev.preventDefault();
  ev.stopPropagation();
  closeTerminalMenu();
  setFocusedLeaf(props.leaf.id);
  tabMenuFor.value = id;
  tabMenuPosition.value = { left: ev.clientX, top: ev.clientY };
  await nextTick();
  positionTabContextMenu();
}

function positionTabContextMenu() {
  const menu = tabMenuRef.value;
  if (!menu) return;
  const rect = menu.getBoundingClientRect();
  const margin = 8;
  const left = Math.min(
    Math.max(margin, tabMenuPosition.value.left),
    window.innerWidth - rect.width - margin,
  );
  const top = Math.min(
    Math.max(margin, tabMenuPosition.value.top),
    window.innerHeight - rect.height - margin,
  );
  tabMenuPosition.value = { left, top };
}

function closeTabContextMenu() {
  tabMenuFor.value = null;
}

function closeCurrentFromMenu() {
  if (!tabMenuFor.value) return;
  void closeTabs([tabMenuFor.value]);
}

function closeOtherTabsFromMenu() {
  if (!tabMenuFor.value) return;
  const targetId = tabMenuFor.value;
  void closeTabs(props.leaf.tabs.filter((id) => id !== targetId));
}

function closeAllTabsFromMenu() {
  void closeTabs([...props.leaf.tabs]);
}

function renameFromMenu() {
  if (!tabMenuFor.value || tabKind(tabMenuFor.value) !== "terminal") return;
  startRename(tabMenuFor.value);
}

// ---- Drag handlers ----

function onTabDragStart(ev: DragEvent, sessionId: string) {
  if (!ev.dataTransfer) return;
  ev.dataTransfer.effectAllowed = "move";
  ev.dataTransfer.setData("text/plain", sessionId);
  drag.begin(sessionId, props.leaf.id);
}

function onTabDragEnd() {
  tabInsertIndex.value = null;
  drag.reset();
}

function onCatcherDragOver(ev: DragEvent) {
  if (!drag.state.active) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "move";
  if (!catcherRef.value) return;
  const rect = catcherRef.value.getBoundingClientRect();
  const zone = computeDropZone(ev.clientX, ev.clientY, rect);
  drag.setHover(props.leaf.id, zone);
}

function onCatcherDragLeave(ev: DragEvent) {
  const related = ev.relatedTarget as Node | null;
  if (catcherRef.value && related && catcherRef.value.contains(related)) return;
  if (drag.state.hoverLeafId === props.leaf.id) drag.setHover(null, null);
}

function onCatcherDrop(ev: DragEvent) {
  if (!drag.state.active || !drag.state.sessionId) return;
  ev.preventDefault();
  const ws = activeWorkspace.value;
  if (!ws) return;
  const sessionId = drag.state.sessionId;
  const zone = drag.state.hoverZone ?? "center";

  if (zone === "center") {
    if (drag.state.sourceLeafId === props.leaf.id) {
      drag.reset();
      return;
    }
    const newRoot = moveTabToLeaf(ws.layout, sessionId, props.leaf.id);
    if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
  } else {
    const split = zoneToSplit(zone);
    if (split) {
      // Edge drop creates a new leaf. Net delta is +1 only if the source leaf survives
      // (has other tabs); otherwise it collapses away.
      const willGainLeaf = sourceSurvivesAfterMove(ws.layout, sessionId);
      if (willGainLeaf && leafCount(ws.layout) >= MAX_PANES) {
        alert(t("You can split into at most {count} panes.", { count: MAX_PANES }));
        drag.reset();
        return;
      }
      const newRoot = moveTabAsSplit(
        ws.layout,
        sessionId,
        props.leaf.id,
        split.direction,
        split.position,
      );
      if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
    }
  }
  setFocusedLeaf(findLeafIdOfSession(ws, sessionId) ?? props.leaf.id);
  drag.reset();
}

function sourceSurvivesAfterMove(root: any, sessionId: string): boolean {
  const stack: any[] = [root];
  while (stack.length) {
    const n = stack.pop();
    if (n.kind === "leaf" && n.tabs.includes(sessionId)) {
      return n.tabs.length > 1;
    }
    if (n.kind === "split") stack.push(...n.children);
  }
  return false;
}

function findLeafIdOfSession(ws: any, sessionId: string): string | null {
  const stack: any[] = [ws.layout];
  while (stack.length) {
    const n = stack.pop();
    if (n.kind === "leaf" && n.tabs.includes(sessionId)) return n.id;
    if (n.kind === "split") stack.push(...n.children);
  }
  return null;
}

// Tab-bar reorder: dragover on a tab placeholder
function onTabBarDragLeave(ev: DragEvent) {
  if (ev.relatedTarget instanceof Node && (ev.currentTarget as HTMLElement).contains(ev.relatedTarget)) return;
  tabInsertIndex.value = null;
}
function onTabBarDragOver(ev: DragEvent) {
  if (!drag.state.active) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "move";
  drag.setHover(null, null);
  const tabs = Array.from((ev.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(".tab"));
  const index = tabs.findIndex(tab => {
    const rect = tab.getBoundingClientRect();
    return ev.clientX < rect.left + rect.width / 2;
  });
  tabInsertIndex.value = index < 0 ? props.leaf.tabs.length : index;
}

function onTabDropAtIndex(ev: DragEvent, atIndex: number) {
  if (!drag.state.active || !drag.state.sessionId) return;
  ev.preventDefault();
  ev.stopPropagation();
  const ws = activeWorkspace.value;
  if (!ws) return;
  const sessionId = drag.state.sessionId;
  const newRoot = moveTabToIndex(ws.layout, sessionId, props.leaf.id, atIndex);
  if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
  setFocusedLeaf(props.leaf.id);
  drag.reset();
}

function onPaneClick() {
  setFocusedLeaf(props.leaf.id);
}

async function onAddClick(ev: MouseEvent) {
  ev.stopPropagation();
  closeTerminalMenu();
  setFocusedLeaf(props.leaf.id);
  await create();
}

async function toggleTerminalMenu(ev: MouseEvent) {
  ev.stopPropagation();
  closeTabContextMenu();
  setFocusedLeaf(props.leaf.id);
  profileMenuAgent.value = null;
  terminalMenuOpen.value = !terminalMenuOpen.value;
  if (!terminalMenuOpen.value) return;
  await nextTick();
  positionTerminalMenu();
}

function positionTerminalMenu() {
  const button = terminalMenuButtonRef.value;
  const menu = terminalMenuRef.value;
  if (!button || !menu) return;
  const buttonRect = button.getBoundingClientRect();
  const menuRect = menu.getBoundingClientRect();
  const margin = 8;
  const left = Math.min(
    Math.max(margin, buttonRect.right - menuRect.width),
    window.innerWidth - menuRect.width - margin,
  );
  const below = buttonRect.bottom + 4;
  const top = below + menuRect.height <= window.innerHeight - margin
    ? below
    : Math.max(margin, buttonRect.top - menuRect.height - 4);
  terminalMenuPosition.value = { left, top };
}

function closeTerminalMenu() {
  profileMenuAgent.value = null;
  terminalMenuOpen.value = false;
}

async function createWithTerminal(terminal: TerminalConfig) {
  closeTerminalMenu();
  setFocusedLeaf(props.leaf.id);
  await create({ terminal: cloneTerminalConfig(terminal) });
}

function onDocumentPointerDown(ev: PointerEvent) {
  const target = ev.target as Node | null;
  if (tabMenuFor.value) {
    if (target && tabMenuRef.value?.contains(target)) {
      return;
    }
    closeTabContextMenu();
  }
  if (!terminalMenuOpen.value) return;
  if (target && (
    profileMenuRef.value?.contains(target)
    || terminalMenuRef.value?.contains(target)
    || terminalMenuButtonRef.value?.contains(target)
  )) return;
  closeTerminalMenu();
}

function onWindowKeyDown(ev: KeyboardEvent) {
  if (ev.key === "Escape" && (terminalMenuOpen.value || tabMenuFor.value)) {
    ev.stopPropagation();
    if (profileMenuAgent.value) {
      closeProfileMenu();
      return;
    }
    closeTerminalMenu();
    closeTabContextMenu();
  }
}

function closeFloatingMenus() {
  closeTerminalMenu();
  closeTabContextMenu();
}

onMounted(() => {
  window.addEventListener("pointerdown", onDocumentPointerDown, true);
  window.addEventListener("keydown", onWindowKeyDown, true);
  window.addEventListener("resize", closeFloatingMenus);
  window.addEventListener("scroll", closeFloatingMenus, true);
});

onUnmounted(() => {
  window.removeEventListener("pointerdown", onDocumentPointerDown, true);
  window.removeEventListener("keydown", onWindowKeyDown, true);
  window.removeEventListener("resize", closeFloatingMenus);
  window.removeEventListener("scroll", closeFloatingMenus, true);
});
</script>

<template>
  <div class="pane" :class="{ focused: isFocused }" @mousedown="onPaneClick">
    <div ref="titlebarRow" class="tab-row" data-tauri-drag-region>
    <div class="tab-bar" :style="titlebarStyle" @dragover="onTabBarDragOver" @dragleave="onTabBarDragLeave" @drop="onTabDropAtIndex($event, tabInsertIndex ?? leaf.tabs.length)">
      <template v-for="(id, i) in leaf.tabs" :key="id">
        <div
          class="drop-gap"
          :class="{ 'insert-active': drag.state.active && tabInsertIndex === i }"
        />
        <div
          :class="['tab', { active: id === leaf.activeTabId }]"
          :draggable="editingId !== id"
          @click="selectTab(id)"
          @dblclick="startRename(id)"
          @mousedown="onTabMouseDown"
          @auxclick="onTabAuxClick($event, id)"
          @contextmenu="openTabContextMenu($event, id)"
          @dragstart="onTabDragStart($event, id)"
          @dragend="onTabDragEnd"
        >
          <template v-if="editingId === id">
            <input
              v-model="editValue"
              autofocus
              @blur="commitRename"
              @keydown.enter="commitRename"
              @keydown.escape="editingId = null"
              @click.stop
              @mousedown.stop
            />
          </template>
          <template v-else>
            <span
              v-if="tabKind(id) === 'terminal'"
              :class="['agent-status', sessionIndicatorClass(sessionAgentStatus(id))]"
            />
            <Icon
              v-if="tabKind(id) === 'terminal'"
              :class="['kind-icon', `agent-${getSessionById(id)?.agent ?? 'terminal'}`]"
              :icon="terminalTabIcon(id)"
            />
            <span v-else class="kind-icon">{{ tabIcon(id) }}</span>
            <SessionProfileTag v-if="tabKind(id) === 'terminal'" :session-id="id" />
            <span class="name">{{ sessionName(id) }}</span>
            <span
              v-if="isFileDirty(id)"
              class="dirty-dot"
              :title="t('Unsaved changes')"
            >●</span>
          </template>
          <span class="close" @click="closeTab(id, $event)">×</span>
        </div>
      </template>
      <div
        class="drop-gap"
        :class="{ 'insert-active': drag.state.active && tabInsertIndex === leaf.tabs.length }"
      />
      <div class="add-terminal">
        <button class="add-tab" :title="t('New terminal (Ctrl+N)')" @click="onAddClick">+</button>
        <button
          ref="terminalMenuButtonRef"
          class="terminal-menu-toggle"
          :class="{ active: terminalMenuOpen }"
          :title="t('Select terminal')"
          :aria-label="t('Select terminal')"
          :aria-expanded="terminalMenuOpen"
          @click="toggleTerminalMenu"
        >
          ▾
        </button>
      </div>
      <div class="tab-bar-spacer" data-tauri-drag-region />
    </div>
    </div>

    <Teleport to="body">
      <div
        v-if="terminalMenuOpen"
        ref="terminalMenuRef"
        class="terminal-picker"
        :style="{
          left: `${terminalMenuPosition.left}px`,
          top: `${terminalMenuPosition.top}px`,
        }"
        role="menu"
        @mousedown.stop
      >
        <div v-for="agent in CLI_AGENTS" :key="agent.id">
          <button class="terminal-option" role="menuitem"
            :aria-haspopup="profilesForAgent(agent.id).length ? 'menu' : undefined"
            :aria-expanded="profileMenuAgent === agent.id"
            @mouseenter="profilesForAgent(agent.id).length ? openProfileMenu(agent.id, $event) : profileMenuAgent = null"
            @click="profilesForAgent(agent.id).length ? openProfileMenu(agent.id, $event, true) : createWithAgent(agent.id)"
            @keydown.right.prevent="profilesForAgent(agent.id).length && openProfileMenu(agent.id, $event, true)">
            <span class="terminal-agent-slot">
              <Icon :class="['terminal-agent-icon', `agent-${agent.id}`]" :icon="sessionAgentIcon(agent.id)" />
            </span>
            <span class="terminal-option-name">{{ agent.id === 'claude' ? 'Claude' : 'Codex' }}</span>
            <span v-if="profilesForAgent(agent.id).length" class="submenu-arrow">&#8250;</span>
          </button>
        </div>
        <div class="terminal-option-separator" />
        <button
          class="terminal-option default-option"
          @mouseenter="profileMenuAgent = null"
          role="menuitem"
          @click="createWithTerminal(effectiveTerminal)"
        >
          <span class="terminal-option-icon">{{ terminalGlyph(effectiveTerminal) }}</span>
          <span class="terminal-option-copy">
            <span class="terminal-option-name">
              {{ terminalLabel(effectiveTerminal) }}
              <span class="default-badge">{{ t("Default") }}</span>
            </span>
          </span>
        </button>
        <div v-if="selectableTerminals.length" class="terminal-option-separator" />
        <button
          v-for="terminal in selectableTerminals"
          :key="terminal.preset"
          @mouseenter="profileMenuAgent = null"
          class="terminal-option"
          role="menuitem"
          @click="createWithTerminal(terminal)"
        >
          <span class="terminal-option-icon">{{ terminalGlyph(terminal) }}</span>
          <span class="terminal-option-copy">
            <span class="terminal-option-name">{{ t(terminal.label) }}</span>
          </span>
        </button>
      </div>
      <div v-if="terminalMenuOpen && profileMenuAgent" ref="profileMenuRef"
        class="terminal-picker profile-picker" role="menu"
        :aria-label="profileMenuAgent === 'claude' ? 'Claude' : 'Codex'"
        :style="{ left: profileMenuPosition.left + 'px', top: profileMenuPosition.top + 'px' }"
        @mousedown.stop @keydown.left.prevent.stop="closeProfileMenu" @keydown.esc.prevent.stop="closeProfileMenu">
        <button class="terminal-option" role="menuitem" @click="createWithAgent(profileMenuAgent, null)">
          <span class="profile-option-label">{{ t('System') }}</span>
          <span v-if="!resolveDefaultProfile(profileMenuAgent, useAccountProfiles().profiles, prefs.defaultProfileId)" class="default-badge">{{ t('Default') }}</span>
        </button>
        <button v-for="profile in profilesForAgent(profileMenuAgent)" :key="profile.id"
          class="terminal-option" role="menuitem" :title="profile.label"
          @click="createWithAgent(profileMenuAgent, profile)">
          <span class="profile-option-label">{{ profile.label }}</span>
          <span v-if="prefs.defaultProfileId[profileMenuAgent] === profile.id" class="default-badge">{{ t('Default') }}</span>
        </button>
      </div>
    </Teleport>

    <Teleport to="body">
      <div
        v-if="tabMenuFor"
        ref="tabMenuRef"
        class="tab-context-menu"
        :style="{
          left: `${tabMenuPosition.left}px`,
          top: `${tabMenuPosition.top}px`,
        }"
        role="menu"
        @mousedown.stop
        @contextmenu.prevent
      >
        <button
          class="tab-menu-item"
          :class="{ disabled: tabKind(tabMenuFor) !== 'terminal' }"
          role="menuitem"
          :disabled="tabKind(tabMenuFor) !== 'terminal'"
          @click="renameFromMenu"
        >{{ t("Rename") }}</button>
        <div class="tab-menu-separator" />
        <button class="tab-menu-item" role="menuitem" @click="closeAllTabsFromMenu">{{ t("Close All Tabs") }}</button>
        <button class="tab-menu-item" role="menuitem" @click="closeCurrentFromMenu">{{ t("Close Current Tab") }}</button>
        <button
          class="tab-menu-item"
          :class="{ disabled: leaf.tabs.length <= 1 }"
          role="menuitem"
          :disabled="leaf.tabs.length <= 1"
          @click="closeOtherTabsFromMenu"
        >{{ t("Close Other Tabs") }}</button>
      </div>
    </Teleport>

    <div class="body">
      <TerminalView
        v-if="leaf.activeTabId && tabKind(leaf.activeTabId) === 'terminal'"
        :key="leaf.activeTabId"
        :session-id="leaf.activeTabId"
        :active="isFocused"
      />
      <FileViewer
        v-if="fileTab(leaf.activeTabId)"
        :key="leaf.activeTabId ?? 'file'"
        :preview="fileTab(leaf.activeTabId)!.preview"
        :tab-id="fileTab(leaf.activeTabId)!.id"
      />
      <template v-for="id in leaf.tabs" :key="'browser-' + id">
        <BrowserView
          v-if="browserTab(id)"
          :tab="browserTab(id)!"
          :active="id === leaf.activeTabId"
        />
      </template>
      <div v-if="!leaf.activeTabId" class="empty">{{ t("No tab in this pane.") }}</div>

      <div
        v-if="drag.state.active"
        ref="catcherRef"
        class="drop-catcher"
        @dragenter.prevent
        @dragover="onCatcherDragOver"
        @dragleave="onCatcherDragLeave"
        @drop="onCatcherDrop"
      >
        <div
          v-if="drag.state.hoverLeafId === leaf.id && drag.state.hoverZone"
          class="overlay"
          :class="['zone-' + drag.state.hoverZone]"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.pane {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  min-width: 0;
  min-height: 0;
  background: #1e1e1e;
  position: relative;
}
.pane.focused .tab-bar {
  border-bottom-color: #4ec9b0;
}
.tab-row { height: var(--titlebar-height); padding-top: 6px; flex-shrink: 0; min-width: 0; overflow: hidden; background: #252525; }
.tab-bar {
  display: flex;
  align-items: stretch;
  height: 100%;
  background: #252525;
  border-bottom: 1px solid #111;
  user-select: none;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
  flex-shrink: 0;
}
.tab-bar::-webkit-scrollbar { display: none; width: 0; height: 0; }
.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  color: #aaa;
  border-right: 1px solid #111;
  cursor: pointer;
  font-size: 12px;
  white-space: nowrap;
}
.tab:hover { background: #2e2e2e; }
.tab.active {
  background: #1e1e1e;
  color: #e6e6e6;
}
.kind-icon {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  width: 1em;
  height: 1em;
  color: #4ec9b0;
  font-family: Consolas, monospace;
  font-size: 11px;
  line-height: 1;
}
.kind-icon.agent-codex {
  font-size: 17px;
}
.kind-icon.agent-claude {
  color: #d97757;
}
.agent-status {
  width: 6px;
  height: 6px;
  flex-shrink: 0;
  border-radius: 50%;
  background: #22c55e;
}
.agent-status.is-working {
  background: #e2b341;
  animation: agent-working 900ms ease-in-out infinite alternate;
}
.agent-status.is-completed { background: #22c55e; }
.agent-status.is-error { background: #e06c75; }
@keyframes agent-working {
  from { opacity: 0.35; transform: scale(0.8); }
  to { opacity: 1; transform: scale(1); }
}
.dirty-dot {
  color: #4ec9b0;
  font-size: 10px;
  line-height: 1;
}
.close {
  opacity: 0.4;
  padding: 0 4px;
  border-radius: 3px;
}
.close:hover {
  opacity: 1;
  background: #5a2d2d;
  color: #fff;
}
.drop-gap.insert-active { background: #4ec9b0; }
.drop-gap {
  width: 4px;
  flex-shrink: 0;
}
.add-terminal {
  display: flex;
  align-items: stretch;
  flex-shrink: 0;
  border-right: 1px solid #111;
}
.add-tab,
.terminal-menu-toggle {
  background: transparent;
  border: none;
  color: #888;
  height: 28px;
  line-height: 1;
  cursor: pointer;
  flex-shrink: 0;
}
.add-tab {
  width: 26px;
  padding: 0 0 1px;
  font-size: 16px;
}
.terminal-menu-toggle {
  width: 18px;
  padding: 0 2px 1px 0;
  font-size: 11px;
}
.add-tab:hover,
.terminal-menu-toggle:hover,
.terminal-menu-toggle.active {
  background: #2e2e2e;
  color: #e6e6e6;
}
.terminal-menu-toggle:focus-visible,
.add-tab:focus-visible {
  outline: 1px solid #4ec9b0;
  outline-offset: -1px;
}
.tab-bar-spacer { flex: 1; min-width: 24px; }
input {
  background: #1e1e1e;
  color: #e6e6e6;
  border: 1px solid #4ec9b0;
  font: inherit;
  padding: 0 4px;
  width: 120px;
}
.body {
  position: relative;
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
}
.body :deep(.term-host) {
  position: absolute;
  inset: 0;
}
.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #666;
  font-size: 12px;
}
.drop-catcher {
  position: absolute;
  inset: 0;
  z-index: 10;
}
.overlay {
  position: absolute;
  background: rgba(78, 201, 176, 0.22);
  border: 2px solid #4ec9b0;
  pointer-events: none;
  transition: all 80ms ease;
}
.overlay.zone-center { inset: 0; }
.overlay.zone-left { top: 0; bottom: 0; left: 0; width: 50%; }
.overlay.zone-right { top: 0; bottom: 0; right: 0; width: 50%; }
.overlay.zone-top { left: 0; right: 0; top: 0; height: 50%; }
.overlay.zone-bottom { left: 0; right: 0; bottom: 0; height: 50%; }

.terminal-picker {
  position: fixed;
  z-index: 1000;
  width: 240px;
  max-width: calc(100vw - 16px);
  max-height: min(420px, calc(100vh - 16px));
  overflow-y: auto;
  padding: 5px;
  background: #252525;
  border: 1px solid #111;
  border-radius: 5px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.58);
  color: #d4d4d4;
  font-size: 12px;
}
.terminal-option {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 30px;
  padding: 6px 8px;
  color: inherit;
  text-align: left;
  background: transparent;
  border: 0;
  border-radius: 3px;
  cursor: pointer;
}
.terminal-option:hover,
.terminal-option:focus-visible {
  background: #094771;
  color: #fff;
  outline: none;
}
.terminal-option-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 20px;
  flex: 0 0 30px;
  color: #4ec9b0;
  background: #1e1e1e;
  border: 1px solid #3a3a3a;
  border-radius: 4px;
  font: 600 10px/1 Consolas, "Cascadia Mono", monospace;
}
.terminal-agent-slot {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 24px;
  flex: 0 0 30px;
}
.terminal-agent-icon { width: 16px; height: 16px; }
.terminal-agent-icon.agent-claude { color: #d97757; }
/* The Codex SVG includes padding around its artwork. */
.terminal-agent-icon.agent-codex { width: 24px; height: 24px; }
.submenu-arrow { margin-left: auto; font-size: 18px; }
.profile-picker { z-index: 1001; }
.profile-option-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.terminal-option-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 3px;
}
.terminal-option-name {
  display: flex;
  align-items: center;
  gap: 6px;
  color: #e6e6e6;
  font-weight: 500;
}

.default-badge {
  padding: 1px 5px;
  color: #9fe3d4;
  background: rgba(78, 201, 176, 0.14);
  border: 1px solid rgba(78, 201, 176, 0.32);
  border-radius: 8px;
  font-size: 9px;
  font-weight: 600;
}
.terminal-option-separator {
  height: 1px;
  margin: 4px 6px;
  background: #151515;
}

.tab-context-menu {
  position: fixed;
  z-index: 1000;
  min-width: 172px;
  padding: 5px;
  background: #252525;
  border: 1px solid #111;
  border-radius: 5px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.58);
  color: #d4d4d4;
  font-size: 12px;
}
.tab-menu-item {
  display: block;
  width: 100%;
  padding: 7px 10px;
  color: inherit;
  text-align: left;
  background: transparent;
  border: 0;
  border-radius: 3px;
  cursor: pointer;
  font: inherit;
}
.tab-menu-item:hover,
.tab-menu-item:focus-visible {
  background: #094771;
  color: #fff;
  outline: none;
}
.tab-menu-item.disabled,
.tab-menu-item:disabled {
  color: #666;
  cursor: not-allowed;
}
.tab-menu-item.disabled:hover,
.tab-menu-item:disabled:hover {
  background: transparent;
  color: #666;
}
.tab-menu-separator {
  height: 1px;
  margin: 4px 6px;
  background: #151515;
}
</style>
