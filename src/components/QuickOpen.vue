<script setup lang="ts">
import { t } from "../composables/useI18n";
import { computed, nextTick, ref, watch } from "vue";
import { Icon, type IconifyIcon } from "@iconify/vue";
import { useQuickOpen } from "../composables/useQuickOpen";
import { useWorkspaces } from "../composables/useWorkspaces";
import { useSessions } from "../composables/useSessions";
import { useFocus } from "../composables/useFocus";
import { usePalette } from "../composables/usePalette";
import { useResources } from "../composables/useResources";
import { activateSessionTab } from "../composables/useLayout";
import {
  rankQuickOpen,
  workspaceCandidates,
  sessionCandidates,
  commandCandidates,
  fileCandidates,
  type QuickOpenCandidate,
  type QuickOpenResult,
  type QuickOpenKind,
} from "../lib/quick-open";
import { resolveExplorerRoot } from "../lib/explorer-root";
import {
  workspaceIcon,
  terminalIcon,
  commandIcon,
  fileIcon,
} from "../lib/offline-icons";
import { api, stringToBase64 } from "../lib/tauri";

const { state, close, setQuery, moveSelection, setSelectedIndex } = useQuickOpen();
const { state: wsState, activeWorkspace, setActiveWorkspace } = useWorkspaces();
const { workspaceSessions, focusedSession, currentCwd } = useSessions();
const { setFocusedLeaf } = useFocus();
const { items } = usePalette();
const resources = useResources();

const inputEl = ref<HTMLInputElement | null>(null);
const listEl = ref<HTMLElement | null>(null);

const KIND_ICONS: Record<QuickOpenKind, IconifyIcon> = {
  workspace: workspaceIcon,
  session: terminalIcon,
  command: commandIcon,
  file: fileIcon,
};

// The file index for the active Workspace's resolved Explorer root. Files are
// bounded (backend cap) and stay hidden until the user types, so a one-shot
// fetch when the overlay opens is enough; the ranking seam owns the empty-query
// rule and the per-kind cap.
const files = ref<QuickOpenCandidate[]>([]);

async function loadFileIndex(): Promise<void> {
  files.value = [];
  const ws = activeWorkspace.value;
  const focused = focusedSession.value;
  const root = resolveExplorerRoot({
    pinnedRoot: ws?.settings?.explorerRoot,
    defaultCwd: ws?.settings?.defaultCwd,
    focusedCwd: focused ? currentCwd(focused.id) : undefined,
  });
  if (!root) return;
  try {
    const index = await api.listFiles(root);
    files.value = fileCandidates(index.files);
  } catch (e) {
    console.warn("listFiles failed", e);
  }
}

// Candidate arrays are built from the live sources; ranking, the empty-query
// rule and the per-kind cap all live in the pure `quick-open` seam.
const candidates = computed<QuickOpenCandidate[]>(() => [
  ...workspaceCandidates(wsState.workspaces),
  ...sessionCandidates(workspaceSessions.value),
  ...files.value,
  ...commandCandidates(items),
]);

const results = computed<QuickOpenResult[]>(() =>
  rankQuickOpen(candidates.value, state.query),
);

function onInput(ev: Event) {
  setQuery((ev.target as HTMLInputElement).value);
}

async function scrollSelectedIntoView() {
  await nextTick();
  listEl.value
    ?.querySelector<HTMLElement>(".qo-result.active")
    ?.scrollIntoView({ block: "nearest" });
}

async function act(result: QuickOpenResult | undefined) {
  if (result) {
    switch (result.kind) {
      case "workspace":
        setActiveWorkspace(result.id);
        break;
      case "session": {
        const ws = activeWorkspace.value;
        const leafId = ws ? activateSessionTab(ws.layout, result.id) : null;
        if (leafId) setFocusedLeaf(leafId);
        break;
      }
      case "command": {
        const item = items.find((it) => it.id === result.id);
        const sid = focusedSession.value?.id;
        // No focused Session → acting on a Command is a no-op (still closes).
        if (item && sid) {
          const text = item.command + (item.autoRun ? "\r" : "");
          if (text) {
            try {
              await api.writeSession(sid, stringToBase64(text));
            } catch (e) {
              console.warn("writeSession failed", e);
            }
          }
        }
        break;
      }
      case "file":
        // `result.id` is the file's absolute path from the backend index.
        try {
          await resources.openFile(result.id);
        } catch (e) {
          console.warn("Failed to open file", e);
        }
        break;
    }
  }
  close();
}

function onKeydown(ev: KeyboardEvent) {
  if (ev.key === "ArrowDown") {
    ev.preventDefault();
    moveSelection(1, results.value.length);
    void scrollSelectedIntoView();
  } else if (ev.key === "ArrowUp") {
    ev.preventDefault();
    moveSelection(-1, results.value.length);
    void scrollSelectedIntoView();
  } else if (ev.key === "Enter") {
    ev.preventDefault();
    void act(results.value[state.selectedIndex]);
  } else if (ev.key === "Escape") {
    ev.preventDefault();
    close();
  }
}

// On open: fetch the file index for the active Workspace and auto-focus the
// text field. On close: drop the index so it is re-fetched fresh next time
// (Quick Open state is ephemeral).
watch(
  () => state.open,
  async (isOpen) => {
    if (!isOpen) {
      files.value = [];
      return;
    }
    void loadFileIndex();
    await nextTick();
    inputEl.value?.focus();
  },
);
</script>

<template>
  <div v-if="state.open" class="qo-backdrop" @mousedown.self="close">
    <div class="qo-panel" role="dialog" :aria-label="t('Quick Open')">
      <input
        ref="inputEl"
        class="qo-input"
        type="text"
        :value="state.query"
        :placeholder="t('Search workspaces, sessions, files, commands…')"
        spellcheck="false"
        autocomplete="off"
        @input="onInput"
        @keydown="onKeydown"
      />
      <ul v-if="results.length" ref="listEl" class="qo-list" role="listbox">
        <li
          v-for="(result, i) in results"
          :key="result.kind + ':' + result.id"
          class="qo-result"
          :class="{ active: i === state.selectedIndex }"
          role="option"
          :aria-selected="i === state.selectedIndex"
          @mouseenter="setSelectedIndex(i)"
          @click="act(result)"
        >
          <Icon class="qo-icon" :icon="KIND_ICONS[result.kind]" />
          <span class="qo-label">{{ result.label }}</span>
          <span v-if="result.subtitle" class="qo-subtitle">{{ result.subtitle }}</span>
          <span class="qo-kind">{{ t(result.kind) }}</span>
        </li>
      </ul>
      <div v-else class="qo-empty">{{ t("No matches") }}</div>
    </div>
  </div>
</template>

<style scoped>
.qo-backdrop {
  position: fixed;
  inset: 0;
  z-index: 900;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 12vh;
  background: rgba(0, 0, 0, 0.35);
}
.qo-panel {
  display: flex;
  flex-direction: column;
  width: 560px;
  max-width: calc(100vw - 32px);
  max-height: 70vh;
  overflow: hidden;
  background: #252525;
  border: 1px solid #111;
  border-radius: 6px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.6);
}
.qo-input {
  flex: 0 0 auto;
  height: 40px;
  padding: 0 12px;
  color: #f0f0f0;
  font-size: 14px;
  background: #1e1e1e;
  border: 0;
  border-bottom: 1px solid #111;
  outline: none;
}
.qo-input::placeholder {
  color: #777;
}
.qo-list {
  flex: 1 1 auto;
  margin: 0;
  padding: 4px;
  overflow-y: auto;
  list-style: none;
}
.qo-result {
  display: flex;
  align-items: center;
  gap: 9px;
  height: 32px;
  padding: 0 10px;
  color: #d4d4d4;
  border-radius: 3px;
  cursor: pointer;
}
.qo-result.active {
  color: #fff;
  background: #094771;
}
.qo-icon {
  flex: 0 0 16px;
  font-size: 16px;
  color: var(--accent);
}
.qo-label {
  flex: 0 1 auto;
  overflow: hidden;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.qo-subtitle {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  font-size: 11px;
  color: #888;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.qo-kind {
  flex: 0 0 auto;
  margin-left: auto;
  padding-left: 8px;
  font-size: 10px;
  color: #6a6a6a;
  text-transform: capitalize;
}
.qo-empty {
  padding: 16px 12px;
  color: #888;
  font-size: 13px;
  text-align: center;
}
</style>
