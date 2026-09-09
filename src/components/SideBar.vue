<script setup lang="ts">
import { t } from "../composables/useI18n";
import { ref, computed, nextTick, onMounted, onBeforeUnmount } from "vue";
import { Icon } from "@iconify/vue";
import SessionProfileTag from "./SessionProfileTag.vue";
import { useWorkspaces, workspaceDefaultCwd } from "../composables/useWorkspaces";
import { useSessions } from "../composables/useSessions";
import { useResources } from "../composables/useResources";
import { useFocus } from "../composables/useFocus";
import { useFlowPage } from "../composables/useFlowPage";
import { useSettings } from "../composables/useSettings";
import { useConfirm } from "../composables/useConfirm";
import { displayName } from "../lib/session-names";
import {
  collectAllSessionIds,
  addTabToLeaf,
  findFirstLeaf,
  activateSessionTab,
} from "../composables/useLayout";
import { buildNavigatorTree, reorderSessionIds } from "../lib/navigator";
import {
  bellIcon,
  folderIcon,
  plusIcon,
} from "../lib/offline-icons";
import { sessionAgentIcon } from "../lib/session-agent-icon";
import { sessionIndicatorClass } from "../lib/session-indicator";

const {
  state,
  createWorkspace,
  deleteWorkspace,
  renameWorkspace,
  setActiveWorkspace,
} = useWorkspaces();
const {
  state: sessState,
  focusedSession,
  activity,
  agentTaskStatus,
  createForWorkspace,
  kill,
  rename,
} = useSessions();
const { confirm: confirmSessionDelete } = useConfirm();
const { setFocusedLeaf } = useFocus();
const { openSettings } = useSettings();
const { openFlow } = useFlowPage();
const resources = useResources();

// Grouped Workspace -> Session tree the Navigator renders. Derivation and its
// active/focused flags are unit-tested in `../lib/navigator`; this component
// only renders the result and wires clicks back to focus/activation.
const tree = computed(() =>
  buildNavigatorTree({
    workspaces: state.workspaces,
    sessions: sessState.sessions,
    activeWorkspaceId: state.activeWorkspaceId,
    focusedSessionId: focusedSession.value?.id ?? null,
    activityById: activity,
    agentStatusById: agentTaskStatus,
  }),
);

const editingId = ref<string | null>(null);
const editValue = ref("");
const menuFor = ref<string | null>(null);
const menuPos = ref({ x: 0, y: 0 });
const menuKind = ref<"workspace" | "session">("workspace");
const menuRef = ref<HTMLElement | null>(null);
const editingSessionId = ref<string | null>(null);
const sessionEditValue = ref("");

async function startSessionRename(id: string) {
  closeMenu();
  const session = sessState.sessions.find(s => s.id === id);
  if (!session) return;
  editingSessionId.value = id;
  sessionEditValue.value = displayName(session.name);
  await nextTick();
  const input = document.querySelector<HTMLInputElement>(".session-rename-input");
  input?.focus();
  input?.select();
}

async function commitSessionRename() {
  const id = editingSessionId.value;
  const name = sessionEditValue.value.trim();
  editingSessionId.value = null;
  if (id && name) await rename(id, name);
}

async function removeSession(id: string) {
  closeMenu();
  const session = sessState.sessions.find(s => s.id === id);
  if (!session) return;
  const ok = await confirmSessionDelete({
    message: t('Kill session "{name}"?', { name: displayName(session.name) }),
    confirmLabel: t("Kill"),
    rememberKey: "skipKillSessionConfirm",
  });
  if (ok && sessState.sessions.some(s => s.id === id)) await kill(id);
}

const draggedSession = ref<{ workspaceId: string; sessionId: string } | null>(null);
const sessionDrop = ref<{ workspaceId: string; sessionId: string; after: boolean } | null>(null);
function endSessionDrag() {
  draggedSession.value = null;
  sessionDrop.value = null;
}
function startSessionDrag(ev: DragEvent, workspaceId: string, sessionId: string) {
  if (!ev.dataTransfer) return;
  ev.dataTransfer.effectAllowed = "move";
  ev.dataTransfer.setData("text/plain", sessionId);
  draggedSession.value = { workspaceId, sessionId };
}
function overSession(ev: DragEvent, workspaceId: string, sessionId: string) {
  if (draggedSession.value?.workspaceId !== workspaceId) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "move";
  const rect = (ev.currentTarget as HTMLElement).getBoundingClientRect();
  sessionDrop.value = { workspaceId, sessionId, after: ev.clientY >= rect.top + rect.height / 2 };
}
function dropSession(ev: DragEvent, workspaceId: string, sessionId: string) {
  overSession(ev, workspaceId, sessionId);
  const source = draggedSession.value;
  const target = sessionDrop.value;
  const ws = state.workspaces.find(w => w.id === workspaceId);
  const group = tree.value.find(w => w.id === workspaceId);
  if (source?.workspaceId === workspaceId && target && ws && group) {
    ev.preventDefault();
    ws.sessionOrder = reorderSessionIds(group.sessions.map(s => s.id), source.sessionId, sessionId, target.after);
  }
  endSessionDrag();
}
function sessionDropClass(workspaceId: string, sessionId: string) {
  const target = sessionDrop.value;
  if (!target || target.workspaceId !== workspaceId || target.sessionId !== sessionId) return "";
  return target.after ? "insert-after" : "insert-before";
}
function activate(id: string) {
  setActiveWorkspace(id);
  closeMenu();
}

// Click-to-focus a Session from the Navigator: switch to its Workspace if
// needed, activate its tab within its leaf, and mark that leaf focused —
// mirroring the focus path used elsewhere (e.g. App.vue's focusSessionByIndex).
function focusSession(workspaceId: string, sessionId: string) {
  const ws = state.workspaces.find((w) => w.id === workspaceId);
  if (!ws) return;
  if (state.activeWorkspaceId !== workspaceId) setActiveWorkspace(workspaceId);
  const leafId = activateSessionTab(ws.layout, sessionId);
  if (leafId === null) return;
  setFocusedLeaf(leafId);
  closeMenu();
}

function addWorkspace() {
  const ws = createWorkspace(`WS${state.workspaces.length}`);
  editingId.value = ws.id;
  editValue.value = ws.name;
}

async function addTerminal(workspaceId: string) {
  const ws = state.workspaces.find((workspace) => workspace.id === workspaceId);
  if (!ws) return;

  // A terminal created from the Navigator belongs to the Workspace whose
  // button was clicked, rather than whichever Workspace happened to be active.
  setActiveWorkspace(ws.id);
  const info = await createForWorkspace(ws, { cwd: workspaceDefaultCwd(ws) });
  if (!info) return;

  const leaf = findFirstLeaf(ws.layout);
  addTabToLeaf(ws.layout, leaf.id, info.id);
  setFocusedLeaf(leaf.id);
}

function startRename(id: string) {
  const ws = state.workspaces.find((w) => w.id === id);
  if (!ws) return;
  editingId.value = id;
  editValue.value = ws.name;
  closeMenu();
}

function commitRename() {
  if (editingId.value && editValue.value.trim()) {
    renameWorkspace(editingId.value, editValue.value.trim());
  }
  editingId.value = null;
}

async function removeWorkspace(id: string) {
  closeMenu();
  if (state.workspaces.length <= 1) return;
  const ws = state.workspaces.find((w) => w.id === id);
  if (!ws) return;
  const sessionIds = collectAllSessionIds(ws.layout);
  if (sessionIds.length > 0) {
    const choice = confirm(
      t('Workspace "{name}" has {count} session(s). OK: move them to the previous workspace. Cancel: kill them.', { name: ws.name, count: sessionIds.length }),
    );
    if (choice) {
      const idx = state.workspaces.findIndex((w) => w.id === id);
      const target = state.workspaces[Math.max(0, idx - 1)] ?? state.workspaces[1];
      if (target && target.id !== id) {
        const leaf = findFirstLeaf(target.layout);
        for (const sid of sessionIds) addTabToLeaf(target.layout, leaf.id, sid);
      }
    } else {
      for (const sid of sessionIds) {
        if (resources.getById(sid)) resources.forgetResource(sid);
        else await kill(sid);
      }
    }
  }
  deleteWorkspace(id);
}

async function openContextMenu(ev: MouseEvent, id: string, kind: "workspace" | "session" = "workspace") {
  ev.preventDefault();
  ev.stopPropagation();
  menuKind.value = kind;
  menuFor.value = id;
  menuPos.value = { x: ev.clientX, y: ev.clientY };
  await nextTick();
  const rect = menuRef.value?.getBoundingClientRect();
  if (rect) menuPos.value = {
    x: Math.max(8, Math.min(ev.clientX, window.innerWidth - rect.width - 8)),
    y: Math.max(8, Math.min(ev.clientY, window.innerHeight - rect.height - 8)),
  };
}

function closeMenu() {
  menuFor.value = null;
}

const canDelete = computed(() => state.workspaces.length > 1);

function onOutsidePointerDown(ev: PointerEvent) {
  if (!menuRef.value?.contains(ev.target as Node)) closeMenu();
}
function onMenuKeydown(ev: KeyboardEvent) {
  if (ev.key === "Escape") closeMenu();
}
onMounted(() => {
  document.addEventListener("pointerdown", onOutsidePointerDown);
  document.addEventListener("keydown", onMenuKeydown);
  window.addEventListener("resize", closeMenu);
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onOutsidePointerDown);
  document.removeEventListener("keydown", onMenuKeydown);
  window.removeEventListener("resize", closeMenu);
});

</script>

<template>
  <aside class="sidebar" @click="closeMenu">
    <button class="add" :title="t('Add project')" :aria-label="t('Add project')" @click.stop="addWorkspace">
      <Icon class="ico" :icon="plusIcon" />
    </button>
    <div class="ws-list">
      <div v-for="ws in tree" :key="ws.id" class="ws-group">
        <div
          :class="['ws', { active: ws.isActiveWorkspace }]"
          :title="ws.name"
          @click.stop="activate(ws.id)"
          @dblclick.stop="startRename(ws.id)"
          @contextmenu="openContextMenu($event, ws.id)"
        >
          <div class="bar" />
          <template v-if="editingId === ws.id">
            <input
              v-model="editValue"
              autofocus
              maxlength="20"
              @blur="commitRename"
              @keydown.enter="commitRename"
              @keydown.escape="editingId = null"
              @click.stop
            />
          </template>
          <template v-else>
            <Icon class="ws-icon" :icon="folderIcon" />
            <span class="ws-name">{{ ws.name }}</span>
            <button
              class="open-flow"
              :title="ws.name + ' - Rhyme Flow'"
              :aria-label="ws.name + ' - Rhyme Flow'"
              @click.stop="openFlow(ws.id)"
              @dblclick.stop
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
                <rect x="3" y="3" width="6" height="6" rx="1.5" />
                <rect x="15" y="15" width="6" height="6" rx="1.5" />
                <path d="M6 9v6a3 3 0 0 0 3 3h6M9 6h6a3 3 0 0 1 3 3v6" />
              </svg>
            </button>
            <button
              class="add-terminal"
              :title="t('Add terminal to {name}', { name: ws.name })"
              @click.stop="addTerminal(ws.id)"
            >
              <Icon :icon="plusIcon" />
            </button>
          </template>
        </div>
        <div v-if="ws.sessions.length" class="sessions">
          <div
            v-for="s in ws.sessions"
            :key="s.id"
            :class="['session', sessionDropClass(ws.id, s.id), { focused: s.isFocusedSession }]"
            :title="s.displayName"
            :draggable="editingSessionId !== s.id"
            @dragstart="startSessionDrag($event, ws.id, s.id)"
            @dragend="endSessionDrag"
            @dragover.stop="overSession($event, ws.id, s.id)"
            @dragleave="sessionDrop = null"
            @drop.stop="dropSession($event, ws.id, s.id)"
            @click.stop="focusSession(ws.id, s.id)"
            @contextmenu="openContextMenu($event, s.id, 'session')"
          >
            <span
              :class="['agent-status', sessionIndicatorClass(s.agentStatus)]"
              :title="t(s.agentStatus ?? 'ready')"
            />
            <Icon
              :class="['s-ico', `agent-${s.agent}`]"
              :icon="sessionAgentIcon(s.agent)"
            />
            <SessionProfileTag :session-id="s.id" />
            <input
              v-if="editingSessionId === s.id"
              v-model="sessionEditValue"
              class="session-rename-input"
              :aria-label="t('Rename')"
              @blur="commitSessionRename"
              @keydown.enter.prevent.stop="!$event.isComposing && commitSessionRename()"
              @keydown.escape.prevent.stop="editingSessionId = null"
              @keydown.stop
              @click.stop
              @contextmenu.stop
            />
            <span v-else class="s-name">{{ s.displayName }}</span>
            <Icon
              v-if="s.hasBell"
              class="s-badge bell"
              :icon="bellIcon"
              :title="t('Rang the bell')"
            />
            <span
              v-else-if="s.hasActivity"
              class="s-badge dot"
              :title="t('New output')"
            />
          </div>
        </div>
      </div>
    </div>
    <div class="sidebar-footer">
      <div class="footer-left">
        <button class="settings-button" :title="t('Settings')" @click="openSettings">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">
            <path d="m9 3-1 3-3 1-2 3 2 2-1 3 3 2 1 3h4l1-3 3-1 2-3-2-2 1-3-3-2-1-3Z" />
            <circle cx="10.5" cy="11.5" r="3" />
          </svg>
          <span>{{ t('Settings') }}</span>
        </button>
      </div>
      <div class="footer-right" />
    </div>
    <div
      v-if="menuFor"
      ref="menuRef"
      class="menu"
      role="menu"
      :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }"
      @click.stop
    >
      <button class="menu-item" role="menuitem" @click="menuKind === 'session' ? startSessionRename(menuFor) : startRename(menuFor)">{{ t("Rename") }}</button>
      <button
        class="menu-item"
        role="menuitem"
        :disabled="menuKind === 'workspace' && !canDelete"
        @click="menuKind === 'session' ? removeSession(menuFor) : removeWorkspace(menuFor)"
      >{{ t("Delete") }}</button>
    </div>
  </aside>
</template>

<style scoped>
.session.insert-before { box-shadow: inset 0 2px #4ec9b0; }
.session.insert-after { box-shadow: inset 0 -2px #4ec9b0; }
.sidebar {
  position: relative;
  width: 100%;
  height: 100%;
  background: #1b1b1b;
  border-right: 1px solid #111;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  padding: 6px 8px;
  gap: 6px;
  user-select: none;
  overflow: hidden;
}
.ws-list {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  width: 100%;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.sidebar-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
  min-height: 40px;
  padding-top: 6px;
  border-top: 1px solid #333;
}
.footer-left,
.footer-right {
  display: flex;
  align-items: center;
}
.settings-button {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 32px;
  padding: 0 8px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: #b8b8b8;
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.settings-button:hover {
  background: #333;
  color: #e6e6e6;
}
.settings-button:focus-visible,
.add:focus-visible {
  outline: 1px solid #4ec9b0;
  outline-offset: -1px;
}
.ws-group {
  display: flex;
  flex-direction: column;
}
.ws {
  position: relative;
  width: 100%;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 0 10px;
  gap: 10px;
  border-radius: 6px;
  background: #2a2a2a;
  color: #d4d4d4;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  flex-shrink: 0;
}
.ws:hover { background: #333; }
.ws.active {
  background: #1e1e1e;
  color: #4ec9b0;
}
.ws .bar {
  position: absolute;
  left: -6px;
  top: 8px;
  bottom: 8px;
  width: 4px;
  border-radius: 2px;
  background: transparent;
}
.ws.active .bar { background: #4ec9b0; }
input {
  background: #1e1e1e;
  color: #e6e6e6;
  border: 1px solid #4ec9b0;
  width: 100%;
  text-align: left;
  font: inherit;
  padding: 0;
}
.add {
  width: 100%;
  height: 36px;
  background: transparent;
  border: 1px dashed #444;
  border-radius: 8px;
  color: #888;
  font-size: 18px;
  cursor: pointer;
  flex-shrink: 0;
}
.add:hover {
  color: #e6e6e6;
  border-color: #4ec9b0;
}
.ws-name {
  display: inline;
  color: inherit;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 12px;
  font-weight: 500;
  flex: 1;
  min-width: 0;
}
.ws-icon {
  flex-shrink: 0;
  font-size: 15px;
  opacity: 0.85;
}
.open-flow,
.add-terminal {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: #9a9a9a;
  cursor: pointer;
  flex-shrink: 0;
}
.open-flow:hover,
.add-terminal:hover {
  background: #3a3a3a;
  color: #e6e6e6;
}
.open-flow:focus-visible, .add-terminal:focus-visible { outline: 2px solid #4ec9b0; outline-offset: 1px; }
.add-terminal :deep(svg) {
  font-size: 16px;
}
.sessions {
  display: flex;
  flex-direction: column;
  margin-top: 2px;
  padding-left: 8px;
}
.session {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 8px;
  border-radius: 5px;
  color: #b8b8b8;
  cursor: pointer;
  font-size: 12px;
}
.session:hover {
  background: #262626;
  color: #e6e6e6;
}
.session.focused {
  background: #223532;
  color: #4ec9b0;
}
.session .s-ico {
  flex-shrink: 0;
  font-size: 14px;
  opacity: 0.85;
}
.session .s-ico.agent-codex {
  font-size: 17px;
}
.session .s-ico.agent-claude {
  color: #d97757;
  opacity: 1;
}
.agent-status {
  width: 7px;
  height: 7px;
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
.session .s-name {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.session .s-badge {
  flex-shrink: 0;
  margin-left: auto;
}
.session .s-badge.bell {
  font-size: 13px;
  color: #e2b341;
}
.session .s-badge.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #4ec9b0;
}
/* The focused Session is cleared upstream, but guard against a residual badge. */
.session.focused .s-badge {
  display: none;
}
.menu {
  position: fixed;
  z-index: 100;
  background: #252525;
  border: 1px solid #111;
  border-radius: 4px;
  min-width: 120px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  font-size: 12px;
}
.menu-item {
  display: block;
  width: 100%;
  border: 0;
  background: transparent;
  text-align: left;
  font: inherit;
  padding: 6px 12px;
  color: #d4d4d4;
  cursor: pointer;
}
.menu-item:hover { background: #2e2e2e; }
.menu-item:disabled {
  color: #555;
  cursor: not-allowed;
}
.menu-item:disabled:hover { background: transparent; }
.session-rename-input {
  flex: 1;
  min-width: 0;
  width: 100%;
  background: #1e1e1e;
  color: inherit;
  border: 1px solid #4ec9b0;
  border-radius: 3px;
  padding: 2px 4px;
  font: inherit;
  outline: none;
}
</style>
