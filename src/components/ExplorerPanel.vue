<script setup lang="ts">
import { t } from "../composables/useI18n";
import { computed, onMounted, ref, watch } from "vue";
import { Icon, type IconifyIcon } from "@iconify/vue";
import {
  chevronRightIcon,
  fileIcon,
  filesIcon,
  folderIcon,
  folderOpenIcon,
  syncIcon,
} from "../lib/offline-icons";
import { api } from "../lib/tauri";
import { useWorkspaces } from "../composables/useWorkspaces";
import { useSessions } from "../composables/useSessions";
import { useResources } from "../composables/useResources";
import { resolveExplorerRoot, syncExplorerRoot } from "../lib/explorer-root";
import { FILE_DRAG_MIME } from "../lib/path-insert";
import { resolveRightPanelTab } from "../lib/right-panel-tabs";
import { explorerPanelTitle } from "../lib/explorer-panel-title";

// The Explorer renders a file tree behind the Files icon strip. The tree is
// rooted at the active Workspace's pinned root (see `explorer-root`), loads one
// directory level at a time on demand, and opens a file as a FileViewer tab via
// `useResources.openFile`. Root resolution and the backend directory read are
// unit-tested in `../lib/explorer-root` and the Rust `read_directory` command;
// this component only wires them to the DOM.

const { activeWorkspace, updateWorkspaceSettings } = useWorkspaces();
const { focusedSession, currentCwd } = useSessions();
const resources = useResources();

interface TreeNode {
  name: string;
  path: string;
  isDir: boolean;
  hidden: boolean;
  expanded: boolean;
  loaded: boolean;
  loading: boolean;
  error: string | null;
  children: TreeNode[];
}

const rootNode = ref<TreeNode | null>(null);
const activeTab = ref(resolveRightPanelTab());

const focusedCwd = computed<string | undefined>(() => {
  const s = focusedSession.value;
  return s ? currentCwd(s.id) : undefined;
});

function basename(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const idx = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  const tail = idx >= 0 ? trimmed.slice(idx + 1) : trimmed;
  return tail || trimmed || path;
}

function makeNode(name: string, path: string, isDir: boolean, hidden = false): TreeNode {
  return {
    name,
    path,
    isDir,
    hidden,
    expanded: false,
    loaded: false,
    loading: false,
    error: null,
    children: [],
  };
}

async function loadChildren(node: TreeNode): Promise<void> {
  if (node.loading) return;
  node.loading = true;
  node.error = null;
  try {
    const listing = await api.readDirectory(node.path);
    node.children = listing.entries.map((e) => makeNode(e.name, e.path, e.isDir, e.hidden));
    node.loaded = true;
  } catch (e) {
    node.error = e instanceof Error ? e.message : String(e);
    node.children = [];
  } finally {
    node.loading = false;
  }
}

// (Re)build the tree for a resolved root path. Reads the focused cwd only at the
// moment of establishment (not reactively) so the root stays fixed as the user
// cd's — syncToTerminal is the only cd-driven re-root.
function buildTree(path: string | undefined): void {
  if (!path) {
    rootNode.value = null;
    return;
  }
  const node = makeNode(basename(path), path, true);
  node.expanded = true;
  rootNode.value = node;
  // Load through `rootNode.value` (the reactive proxy), not the raw `node`, so
  // the loaded children and loading/error flags trigger a re-render.
  if (rootNode.value) void loadChildren(rootNode.value);
}

function establishRoot(): void {
  const ws = activeWorkspace.value;
  const resolved = resolveExplorerRoot({
    pinnedRoot: ws?.settings?.explorerRoot,
    defaultCwd: ws?.settings?.defaultCwd,
    focusedCwd: focusedCwd.value,
  });
  // First-open fallback: when a Workspace has neither a pinned root nor a
  // defaultCwd, `resolved` came from the focused Session's live cwd. Pin it so
  // the root stays fixed as the user cd's (and is remembered across restarts)
  // instead of re-deriving from the live cwd the next time this runs.
  if (
    ws
    && resolved
    && !ws.settings?.explorerRoot?.trim()
    && !ws.settings?.defaultCwd?.trim()
  ) {
    updateWorkspaceSettings(ws.id, { explorerRoot: resolved });
  }
  buildTree(resolved);
}

// Establish on first open and re-establish when the active Workspace changes
// (switching Workspaces surfaces that Workspace's pinned root). Not on
// focusedCwd changes — the root must not thrash as the user cd's.
onMounted(establishRoot);
watch(() => activeWorkspace.value?.id, establishRoot);

function syncToTerminal(): void {
  const next = syncExplorerRoot(focusedCwd.value);
  if (!next) return;
  const ws = activeWorkspace.value;
  if (ws) updateWorkspaceSettings(ws.id, { explorerRoot: next });
  buildTree(next);
}

function toggle(node: TreeNode): void {
  if (!node.isDir) return;
  node.expanded = !node.expanded;
  if (node.expanded && !node.loaded) void loadChildren(node);
}

async function activate(node: TreeNode): Promise<void> {
  if (node.isDir) {
    toggle(node);
    return;
  }
  try {
    await resources.openFile(node.path);
  } catch (e) {
    console.warn("Failed to open file", e);
  }
}

// Start a file drag carrying the file's absolute path on a winmux-specific MIME
// type, so dropping it on a Terminal inserts the path (see Terminal.vue) without
// clashing with Pane-tab drags. Directory rows are not draggable. Click-to-open
// still works — a click that isn't a drag falls through to `activate`.
function onRowDragStart(ev: DragEvent, node: TreeNode): void {
  if (node.isDir || !ev.dataTransfer) return;
  ev.dataTransfer.effectAllowed = "copy";
  ev.dataTransfer.setData(FILE_DRAG_MIME, node.path);
}

// Flatten the expanded tree into visible rows with depth, for a simple list
// render (no per-level recursion in the template).
interface Row {
  node: TreeNode;
  depth: number;
}

const rows = computed<Row[]>(() => {
  const out: Row[] = [];
  const walk = (nodes: TreeNode[], depth: number) => {
    for (const node of nodes) {
      out.push({ node, depth });
      if (node.isDir && node.expanded) walk(node.children, depth + 1);
    }
  };
  if (rootNode.value) walk(rootNode.value.children, 0);
  return out;
});

function iconFor(node: TreeNode): IconifyIcon {
  if (!node.isDir) return fileIcon;
  return node.expanded ? folderOpenIcon : folderIcon;
}

const rootName = computed(() => (rootNode.value ? rootNode.value.name : ""));
const title = computed(() => rootName.value ? explorerPanelTitle(rootName.value) : t("Explorer"));
const canSync = computed(() => !!focusedCwd.value);
</script>

<template>
  <section class="explorer">
    <div class="body">
      <nav class="strip" :aria-label="t('Right panel tools')">
        <button
          class="tool"
          :class="{ active: activeTab === 'files' }"
          :aria-current="activeTab === 'files' ? 'page' : undefined"
          :title="t('Files')"
          type="button"
          @click="activeTab = 'files'"
        >
          <Icon class="ico" :icon="filesIcon" />
          <span>{{ t("Files") }}</span>
        </button>
      </nav>

      <div class="head">
        <span class="title">{{ title }}</span>
        <button
          class="sync"
          type="button"
          :title="t('Sync to current terminal')"
          :disabled="!canSync"
          @click="syncToTerminal"
        >
          <Icon class="ico" :icon="syncIcon" />
        </button>
      </div>

      <div class="tree">
        <template v-if="rootNode">
          <div v-if="rootNode.loading && !rootNode.loaded" class="hint"> {{ t("Loading…") }} </div>
          <div v-else-if="rootNode.error" class="hint error">{{ rootNode.error }}</div>
          <div v-else-if="!rows.length" class="hint">{{ t("Empty folder.") }}</div>
          <div
            v-for="{ node, depth } in rows"
            :key="node.path"
            :class="['row', { dir: node.isDir, hidden: node.hidden }]"
            :style="{ paddingLeft: 6 + depth * 12 + 'px' }"
            :title="node.name"
            :draggable="!node.isDir"
            @click="activate(node)"
            @dragstart="onRowDragStart($event, node)"
          >
            <Icon
              v-if="node.isDir"
              class="chevron"
              :class="{ open: node.expanded }"
              :icon="chevronRightIcon"
            />
            <span v-else class="chevron-spacer" />
            <Icon class="ico entry" :icon="iconFor(node)" />
            <span class="entry-name">{{ node.name }}</span>
          </div>
        </template>
        <div v-else class="hint">{{ t("No folder yet. Focus a terminal and press") }} <button class="inline-sync" type="button" :disabled="!canSync" @click="syncToTerminal">{{ t("sync") }}</button> {{ t("to root the tree.") }}</div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.explorer {
  height: 100%;
  width: 100%;
  background: #1b1b1b;
  border-left: 1px solid #111;
  overflow: hidden;
}
.body {
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 0 4px 8px 8px;
}
.strip {
  display: flex;
  align-items: center;
  min-height: 40px;
  border-bottom: 1px solid #111;
  margin-left: -8px;
  padding: 0 6px;
  background: #181818;
}
.tool {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 30px;
  padding: 0 9px;
  border: none;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: #888;
  cursor: pointer;
  font: inherit;
  font-size: 12px;
}
.tool:hover { color: #d4d4d4; background: #242424; }
.tool.active { color: #d9b96a; border-bottom-color: #d9b96a; }
.tool:focus-visible { outline: 1px solid #4ec9b0; outline-offset: -2px; }
.tool .ico { font-size: 17px; }
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-right: 4px;
  margin-top: 8px;
  margin-bottom: 8px;
}
.title {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: #888;
}
.sync {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: #888;
  cursor: pointer;
}
.sync:hover:not(:disabled) { color: #4ec9b0; background: #262626; }
.sync:disabled { opacity: 0.35; cursor: default; }
.sync .ico { font-size: 16px; }
.tree {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}
.row {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding-right: 4px;
  border-radius: 4px;
  color: #c8c8c8;
  cursor: pointer;
  font-size: 12px;
  white-space: nowrap;
}
.row:hover { background: #262626; color: #e6e6e6; }
.row.hidden { opacity: 0.55; }
.chevron {
  flex-shrink: 0;
  font-size: 14px;
  color: #888;
  transition: transform 100ms ease;
}
.chevron.open { transform: rotate(90deg); }
.chevron-spacer {
  flex-shrink: 0;
  width: 14px;
}
.ico.entry {
  flex-shrink: 0;
  font-size: 15px;
}
.row.dir .ico.entry { color: #d9b96a; }
.row:not(.dir) .ico.entry { color: #7aa6c2; }
.entry-name {
  overflow: hidden;
  text-overflow: ellipsis;
}
.hint {
  color: #666;
  font-size: 12px;
  line-height: 1.5;
  padding: 4px;
}
.hint.error { color: #d08770; word-break: break-word; }
.inline-sync {
  border: none;
  background: transparent;
  color: #4ec9b0;
  cursor: pointer;
  padding: 0 2px;
  font: inherit;
  text-decoration: underline;
}
.inline-sync:disabled { color: #666; cursor: default; text-decoration: none; }
</style>
