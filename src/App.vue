<script setup lang="ts">
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, onMounted } from "vue";
import SideBar from "./components/SideBar.vue";
import ExplorerPanel from "./components/ExplorerPanel.vue";
import StatusBar from "./components/StatusBar.vue";
import SplitContainer from "./components/SplitContainer.vue";
import MenuBar from "./components/MenuBar.vue";
import WindowControls from "./components/WindowControls.vue";
import { TITLEBAR_HEIGHT } from "./lib/titlebar-layout";
import SettingsModal from "./components/SettingsModal.vue";
import ConfirmModal from "./components/ConfirmModal.vue";
import PalettePopover from "./components/PalettePopover.vue";
import QuickOpen from "./components/QuickOpen.vue";
import { useSessions, nextDaemonName, displayName } from "./composables/useSessions";
import { useWorkspaces, loadFromStorage, workspaceDefaultCwd } from "./composables/useWorkspaces";
import { useFocus } from "./composables/useFocus";
import { usePrefixKey } from "./composables/usePrefixKey";
import { useKeybindings, loadKeybindingsFromStorage } from "./composables/useKeybindings";
import { useGlobalShortcuts, registerAction, registerFocusSessionByIndex } from "./composables/useGlobalShortcuts";
import { useSettings } from "./composables/useSettings";
import { useConfirm } from "./composables/useConfirm";
import { loadPrefsFromStorage, usePrefs } from "./composables/usePrefs";
import { loadPaletteFromStorage } from "./composables/usePalette";
import {
  loadAccountProfilesFromStorage,
  resolveProfileEnv,
  useAccountProfiles,
} from "./composables/useAccountProfiles";
import { resolveDefaultProfile } from "./lib/default-profile";
import { useResources } from "./composables/useResources";
import { useQuickOpen } from "./composables/useQuickOpen";
import { useShellPanels } from "./composables/useShellPanels";
import { onSessionAgentChanged } from "./lib/tauri";
import { ACTIONS, type ActionId } from "./lib/keybindings";
import {
  activateSessionTab,
  addTabToLeaf,
  collectAllLeaves,
  collectAllSessionIds,
  findFirstLeaf,
  findLeafById,
  findLeafBySession,
  findNeighborLeafId,
  leafCount,
  MAX_PANES,
  moveTabToLeaf,
  pruneMissing,
  quadrantSplitLeaf,
  replaceTabId,
  splitLeaf,
} from "./composables/useLayout";

const {
  state: sessState,
  refresh,
  create,
  createForWorkspace,
  kill,
  focusedSession,
  workspaceSessions,
  rename,
  applyAgentUpdate,
} = useSessions();
const {
  activeWorkspace,
  removeTerminalSnapshot,
  replaceLayout,
  state: wsState,
} = useWorkspaces();
const { focusedLeafId, setFocusedLeaf } = useFocus();
const { settingsOpen, openSettings } = useSettings();
const { prefixFor } = useKeybindings();
const { confirm } = useConfirm();
const resources = useResources();
const { open: openQuickOpen } = useQuickOpen();
const { panels, toggleLeft, toggleRight, resize, commit } = useShellPanels();
const { prefs } = usePrefs();
const { profiles } = useAccountProfiles();
useGlobalShortcuts();

let unlistenSessionAgent: UnlistenFn | null = null;
let agentListenerRetry: ReturnType<typeof setTimeout> | null = null;
let agentListenerStarting: Promise<void> | null = null;
let appUnmounted = false;

function startSessionAgentListener(resyncAfterRetry = false): Promise<void> {
  if (unlistenSessionAgent || appUnmounted) return Promise.resolve();
  if (agentListenerStarting) return agentListenerStarting;

  agentListenerStarting = onSessionAgentChanged(applyAgentUpdate)
    .then(async (unlisten) => {
      if (appUnmounted) {
        unlisten();
      } else {
        unlistenSessionAgent = unlisten;
        if (resyncAfterRetry) {
          try {
            await refresh();
          } catch (error) {
            console.warn("Failed to resync sessions after agent listener retry", error);
          }
        }
      }
    })
    .catch((error) => {
      console.warn("Failed to subscribe to session agent changes", error);
      if (!appUnmounted) {
        agentListenerRetry = setTimeout(() => {
          agentListenerRetry = null;
          void startSessionAgentListener(true);
        }, 1000);
      }
    })
    .finally(() => {
      agentListenerStarting = null;
    });
  return agentListenerStarting;
}

// Between-region splitter drag: resize a panel's width in px while the center
// content flexes to fill the rest. Widths are clamped to the panel's minimum;
// state is persisted once the drag ends (see useShellPanels).
function startRegionResize(side: "left" | "right", ev: MouseEvent) {
  ev.preventDefault();
  const startX = ev.clientX;
  const startWidth = panels[side].width;
  const sign = side === "left" ? 1 : -1;

  function onMove(e: MouseEvent) {
    resize(side, startWidth + sign * (e.clientX - startX));
  }
  function onUp() {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    commit();
  }
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

const leftRegionStyle = computed(() => ({
  width: `${panels.left.width}px`,
}));
const rightRegionStyle = computed(() => ({ width: `${panels.right.width}px` }));

async function bootstrap() {
  loadPrefsFromStorage();
  loadKeybindingsFromStorage();
  loadPaletteFromStorage();
  loadAccountProfilesFromStorage();
  loadFromStorage();
  await refresh();

  const validIds = new Set(sessState.sessions.map((s) => s.id));
  await restorePersistedSessions(validIds);

  // Prune missing sessions from every workspace layout.
  for (const ws of wsState.workspaces) {
    const newRoot = pruneMissing(ws.layout, validIds);
    if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
  }

  // Attach orphan sessions (in daemon but not in any layout) to the active workspace's first leaf.
  const placed = new Set<string>();
  for (const ws of wsState.workspaces) {
    for (const sid of collectAllSessionIds(ws.layout)) placed.add(sid);
  }
  const ws = activeWorkspace.value;
  if (ws) {
    const leaf = findFirstLeaf(ws.layout);
    for (const s of sessState.sessions) {
      if (!placed.has(s.id)) addTabToLeaf(ws.layout, leaf.id, s.id);
    }
    // Set initial focus to the first non-empty leaf with an active tab.
    const firstLeafWithTabs = collectAllLeaves(ws.layout).find((l) => l.tabs.length > 0);
    if (firstLeafWithTabs) {
      setFocusedLeaf(firstLeafWithTabs.id);
      if (!firstLeafWithTabs.activeTabId) {
        firstLeafWithTabs.activeTabId = firstLeafWithTabs.tabs[0];
      }
    } else {
      setFocusedLeaf(leaf.id);
    }
  }

  // Ensure at least one session exists.
  if (sessState.sessions.length === 0) {
    await create();
  }
}

async function restorePersistedSessions(validIds: Set<string>) {
  for (const ws of wsState.workspaces) {
    const ids = [...new Set(collectAllSessionIds(ws.layout))];
    for (const oldId of ids) {
      if (validIds.has(oldId) || resources.getById(oldId)) continue;
      const snapshot = ws.terminalSnapshots?.[oldId];
      if (!snapshot) continue;

      const restored = await createForWorkspace(ws, {
        name: snapshot.name,
        terminal: snapshot.terminal,
        cwd: snapshot.cwd ?? undefined,
        showError: false,
      }) ?? await createForWorkspace(ws, {
        name: snapshot.name,
        showError: false,
      });

      if (!restored) {
        console.warn(`Failed to restore terminal tab ${oldId}`);
        continue;
      }
      if (replaceTabId(ws.layout, oldId, restored.id)) {
        removeTerminalSnapshot(ws.id, oldId);
        validIds.add(restored.id);
      }
    }
  }
}

async function promptRename() {
  const s = focusedSession.value;
  if (!s) return;
  const name = window.prompt("Rename session", displayName(s.name));
  if (name && name.trim()) await rename(s.id, name.trim());
}

async function killFocused() {
  const ws = activeWorkspace.value;
  const leaf = ws && focusedLeafId.value ? findLeafById(ws.layout, focusedLeafId.value) : null;
  if (leaf?.activeTabId && resources.getById(leaf.activeTabId)) {
    resources.closeResource(leaf.activeTabId);
    return;
  }
  const s = focusedSession.value;
  if (!s) return;
  const ok = await confirm({
    message: `Kill session "${displayName(s.name)}"?`,
    confirmLabel: "Kill",
    rememberKey: "skipKillSessionConfirm",
  });
  if (ok) {
    await kill(s.id);
    if (sessState.sessions.length === 0) await create();
  }
}

async function killFocusedNow() {
  const ws = activeWorkspace.value;
  const leaf = ws && focusedLeafId.value ? findLeafById(ws.layout, focusedLeafId.value) : null;
  if (leaf?.activeTabId && resources.getById(leaf.activeTabId)) {
    resources.closeResource(leaf.activeTabId);
    return;
  }
  const s = focusedSession.value;
  if (!s) return;
  await kill(s.id);
  if (sessState.sessions.length === 0) await create();
}

async function detach() {
  const w = getCurrentWebviewWindow();
  await w.hide();
}

function cycleInLeaf(delta: number) {
  const ws = activeWorkspace.value;
  if (!ws || !focusedLeafId.value) return;
  const leaf = findLeafById(ws.layout, focusedLeafId.value);
  if (!leaf || leaf.tabs.length < 2 || !leaf.activeTabId) return;
  const idx = leaf.tabs.indexOf(leaf.activeTabId);
  const n = leaf.tabs[(idx + delta + leaf.tabs.length) % leaf.tabs.length];
  leaf.activeTabId = n;
}

function selectByIndexInLeaf(i: number) {
  const ws = activeWorkspace.value;
  if (!ws || !focusedLeafId.value) return;
  const leaf = findLeafById(ws.layout, focusedLeafId.value);
  if (!leaf) return;
  const tab = leaf.tabs[i];
  if (tab) leaf.activeTabId = tab;
}

async function splitAndCreate(direction: "horizontal" | "vertical") {
  const ws = activeWorkspace.value;
  if (!ws || !focusedLeafId.value) return;
  if (leafCount(ws.layout) >= MAX_PANES) {
    alert(`최대 ${MAX_PANES}개 pane까지 분할할 수 있습니다.`);
    return;
  }
  const info = await createForWorkspace(ws, {
    name: nextDaemonName(ws, sessState.sessions),
    cwd: workspaceDefaultCwd(ws),
  });
  if (!info) return;
  const newRoot = splitLeaf(ws.layout, focusedLeafId.value, info.id, direction, "after");
  if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
}

function focusSessionByGlobalDelta(delta: number) {
  const list = workspaceSessions.value;
  const n = list.length;
  if (n === 0) return;
  const cur = focusedSession.value;
  const idx = cur ? list.findIndex((s) => s.id === cur.id) : -1;
  const base = idx < 0 ? 0 : idx;
  focusSessionByIndex((base + delta + n) % n);
}

async function splitOrMove(dir: "left" | "right" | "up" | "down") {
  const ws = activeWorkspace.value;
  if (!ws || !focusedLeafId.value) return;
  const cur = findLeafById(ws.layout, focusedLeafId.value);
  if (!cur) return;
  const neighborId = findNeighborLeafId(ws.layout, cur.id, dir);
  if (neighborId) {
    const sessionId = cur.activeTabId;
    if (!sessionId) {
      setFocusedLeaf(neighborId);
      return;
    }
    const newRoot = moveTabToLeaf(ws.layout, sessionId, neighborId);
    if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
    setFocusedLeaf(neighborId);
  } else {
    if (leafCount(ws.layout) >= MAX_PANES) {
      alert(`최대 ${MAX_PANES}개 pane까지 분할할 수 있습니다.`);
      return;
    }
    const direction = (dir === "left" || dir === "right") ? "horizontal" : "vertical";
    const position = (dir === "left" || dir === "up") ? "before" : "after";
    const info = await createForWorkspace(ws, {
      name: nextDaemonName(ws, sessState.sessions),
      cwd: workspaceDefaultCwd(ws),
    });
    if (!info) return;
    const newRoot = splitLeaf(ws.layout, cur.id, info.id, direction, position);
    if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
    const newLeaf = findLeafBySession(newRoot, info.id);
    if (newLeaf) setFocusedLeaf(newLeaf.id);
  }
}

async function quadrantSplit(corner: "tl" | "tr" | "bl" | "br") {
  const ws = activeWorkspace.value;
  if (!ws || !focusedLeafId.value) return;
  if (leafCount(ws.layout) + 2 > MAX_PANES) {
    alert(`최대 ${MAX_PANES}개 pane까지 분할할 수 있습니다.`);
    return;
  }
  const a = await createForWorkspace(ws, {
    name: nextDaemonName(ws, sessState.sessions),
    cwd: workspaceDefaultCwd(ws),
  });
  if (!a) return;
  const b = await createForWorkspace(ws, {
    name: nextDaemonName(ws, sessState.sessions),
    cwd: workspaceDefaultCwd(ws),
  });
  if (!b) {
    await kill(a.id);
    return;
  }
  const newRoot = quadrantSplitLeaf(ws.layout, focusedLeafId.value, a.id, b.id, corner);
  if (newRoot !== ws.layout) replaceLayout(ws.id, newRoot);
  const focusLeaf = findLeafBySession(newRoot, a.id);
  if (focusLeaf) setFocusedLeaf(focusLeaf.id);
}

/**
 * Launches a new Claude session using `prefs.defaultProfileId.claude` (set in
 * Settings → Accounts) if one is chosen, or the system account (no env
 * override) otherwise. Backs the "Launch Claude" quick action.
 */
async function launchDefaultClaude() {
  const profile = resolveDefaultProfile("claude", profiles, prefs.defaultProfileId);
  const env = profile ? await resolveProfileEnv(profile) : undefined;
  await create({ env, launchCommand: "claude" });
}

const ACTION_HANDLERS: Record<ActionId, () => void | Promise<void>> = {
  "session.new": async () => { await create(); },
  "session.newClaude": () => launchDefaultClaude(),
  "session.kill": () => killFocused(),
  "session.killNoConfirm": () => killFocusedNow(),
  "session.rename": () => promptRename(),
  "session.cycleNext": () => cycleInLeaf(1),
  "session.cyclePrev": () => cycleInLeaf(-1),
  "pane.splitHorizontal": () => splitAndCreate("horizontal"),
  "pane.splitVertical": () => splitAndCreate("vertical"),
  "window.detach": () => detach(),
  "settings.open": () => openSettings(),
  "session.focusPrev": () => focusSessionByGlobalDelta(-1),
  "session.focusNext": () => focusSessionByGlobalDelta(1),
  "pane.splitOrMoveLeft": () => splitOrMove("left"),
  "pane.splitOrMoveRight": () => splitOrMove("right"),
  "pane.splitOrMoveUp": () => splitOrMove("up"),
  "pane.splitOrMoveDown": () => splitOrMove("down"),
  "pane.quadrantTopLeft": () => quadrantSplit("tl"),
  "pane.quadrantTopRight": () => quadrantSplit("tr"),
  "pane.quadrantBottomLeft": () => quadrantSplit("bl"),
  "pane.quadrantBottomRight": () => quadrantSplit("br"),
  "view.toggleLeftPanel": () => toggleLeft(),
  "view.toggleRightPanel": () => toggleRight(),
  "view.quickOpen": () => openQuickOpen(),
};

for (const a of ACTIONS) {
  const h = ACTION_HANDLERS[a.id as ActionId];
  if (h) registerAction(a.id as ActionId, h);
}

function focusSessionByIndex(i: number) {
  const s = workspaceSessions.value[i];
  if (!s) return;
  const ws = activeWorkspace.value;
  if (!ws) return;
  const leafId = activateSessionTab(ws.layout, s.id);
  if (leafId) setFocusedLeaf(leafId);
}

registerFocusSessionByIndex(focusSessionByIndex);

usePrefixKey(async (key) => {
  // Numeric tab selection is hard-wired (not a configurable action).
  if (/^[0-9]$/.test(key)) {
    selectByIndexInLeaf(parseInt(key, 10));
    return;
  }
  for (const a of ACTIONS) {
    if (prefixFor(a.id as ActionId) === key) {
      const h = ACTION_HANDLERS[a.id as ActionId];
      if (h) await h();
      return;
    }
  }
});

onMounted(async () => {
  await startSessionAgentListener();
  await bootstrap();
});

onBeforeUnmount(() => {
  appUnmounted = true;
  if (agentListenerRetry) clearTimeout(agentListenerRetry);
  unlistenSessionAgent?.();
  unlistenSessionAgent = null;
});
</script>

<template>
  <div class="app" :style="{ '--titlebar-height': `${TITLEBAR_HEIGHT}px` }">
    <MenuBar class="shell-menu" :style="{ width: panels.left.open ? `${panels.left.width}px` : '102px' }" />
    <WindowControls />
    <div class="main">
      <div v-if="panels.left.open" class="region region-left" :style="leftRegionStyle">
        <SideBar />
      </div>
      <div
        v-if="panels.left.open"
        class="region-splitter"
        title="Drag to resize"
        @mousedown="startRegionResize('left', $event)"
      />
      <div class="content">
        <SplitContainer v-if="activeWorkspace" :key="activeWorkspace.id" :node="activeWorkspace.layout" />
      </div>
      <div
        v-if="panels.right.open"
        class="region-splitter"
        title="Drag to resize"
        @mousedown="startRegionResize('right', $event)"
      />
      <div v-if="panels.right.open" class="region region-right" :style="rightRegionStyle">
        <ExplorerPanel />
      </div>
    </div>
    <StatusBar />
    <QuickOpen />
    <SettingsModal v-if="settingsOpen" />
    <ConfirmModal />
    <PalettePopover />
  </div>
</template>

<style>
html, body, #app {
  margin: 0;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: #1e1e1e;
  color: #d4d4d4;
  font-family: Inter, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }

*::-webkit-scrollbar { width: 6px; height: 6px; }
*::-webkit-scrollbar-track { background: transparent; }
*::-webkit-scrollbar-thumb {
  background: #3a3a3a;
  border-radius: 3px;
}
*::-webkit-scrollbar-thumb:hover { background: #555; }
*::-webkit-scrollbar-corner { background: transparent; }
* { scrollbar-width: thin; scrollbar-color: #3a3a3a transparent; }
.xterm-viewport {
  scrollbar-width: thin;
  scrollbar-color: #3a3a3a transparent;
}
</style>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
}
.main {
  display: flex;
  flex: 1;
  min-height: 0;
}
.region {
  flex-shrink: 0;
  min-height: 0;
  overflow: hidden;
  padding-top: var(--titlebar-height);
}
.shell-menu { position: fixed; top: 0; left: 0; }
.region-splitter {
  flex: 0 0 4px;
  background: #111;
  cursor: col-resize;
  z-index: 1;
}
.region-splitter:hover {
  background: #4ec9b0;
}
.content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  position: relative;
  background: #1e1e1e;
}
</style>
