<script setup lang="ts">
import { t } from "../composables/useI18n";
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { Icon, type IconifyIcon } from "@iconify/vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
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
import { useConfirm } from "../composables/useConfirm";
import ExplorerContextMenu from "./ExplorerContextMenu.vue";
import {
  explorerMenuItems,
  relativeDisplayPath,
  validateEntryName,
  type ExplorerAction,
} from "../lib/explorer-menu";
import { resolveExplorerRoot, syncExplorerRoot } from "../lib/explorer-root";
import { FILE_DRAG_MIME } from "../lib/path-insert";
import { resolveRightPanelTab } from "../lib/right-panel-tabs";
import { explorerPanelTitle } from "../lib/explorer-panel-title";

// The Explorer renders a file tree behind the Files icon strip. The tree is
// rooted at the active Workspace's pinned root (see `explorer-root`), loads one
// directory level at a time on demand, and opens a file as a FileViewer tab via
// `useResources.openFile`. Right-clicking a row opens the context menu
// (`explorer-menu`), whose rename/create commands edit the name inline in the
// tree and whose file operations run through the Rust `rename_path` /
// `delete_path` / `create_entry` commands. Root resolution, the menu model, and
// the backend operations are unit-tested in `../lib/explorer-root`,
// `../lib/explorer-menu` and `commands.rs`; this component only wires them up.

const { activeWorkspace, updateWorkspaceSettings } = useWorkspaces();
const { focusedSession, currentCwd } = useSessions();
const resources = useResources();
const { confirm } = useConfirm();

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

/** The open right-click menu, anchored at the pointer. */
interface MenuState {
  node: TreeNode;
  /** True when the click landed on empty space, targeting the tree root. */
  isRoot: boolean;
  x: number;
  y: number;
}
const menu = ref<MenuState | null>(null);

/**
 * The inline name editor: either renaming an existing row, or a draft row for a
 * not-yet-created entry inside `parent`. Only one can be open at a time.
 */
type EditState =
  | { kind: "rename"; path: string; name: string }
  | { kind: "create"; parent: string; isDir: boolean; name: string };
const editing = ref<EditState | null>(null);
const editInput = ref<HTMLInputElement | null>(null);
/** Rejected-name or failed-write message shown under the inline editor. */
const editError = ref<string | null>(null);
/** Failure of a menu command that has no inline editor to report into. */
const opError = ref<string | null>(null);
const busy = ref(false);

function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

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
    // Re-reading after a rename/delete/create (or an explicit Refresh) must not
    // collapse the rest of the subtree, so surviving entries keep their node —
    // and with it their expanded children.
    const previous = new Map(node.children.map((child) => [child.path, child]));
    node.children = listing.entries.map((e) => {
      const kept = previous.get(e.path);
      return kept && kept.isDir === e.isDir ? kept : makeNode(e.name, e.path, e.isDir, e.hidden);
    });
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
  if (resolved === rootNode.value?.path) return;
  buildTree(resolved);
}

// Establish on first open, re-establish when the active Workspace changes
// (switching Workspaces surfaces that Workspace's pinned root), and follow the
// active Workspace's default folder as it's edited in Settings — as long as no
// root is pinned, resolveExplorerRoot falls through to defaultCwd, so this
// keeps the tree in sync with the project's base path. Not on focusedCwd
// changes — the root must not thrash as the user cd's.
onMounted(establishRoot);
watch(() => activeWorkspace.value?.id, establishRoot);
watch(() => activeWorkspace.value?.settings?.defaultCwd, establishRoot);

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

// --- Tree lookups, used to refresh the right subtree after a file operation ---

function findNode(path: string): TreeNode | null {
  const walk = (node: TreeNode): TreeNode | null => {
    if (node.path === path) return node;
    for (const child of node.children) {
      const hit = walk(child);
      if (hit) return hit;
    }
    return null;
  };
  return rootNode.value ? walk(rootNode.value) : null;
}

/** The directory node holding `path`, i.e. the one to re-read after a change. */
function findParentOf(path: string): TreeNode | null {
  const walk = (node: TreeNode): TreeNode | null => {
    for (const child of node.children) {
      if (child.path === path) return node;
      const hit = walk(child);
      if (hit) return hit;
    }
    return null;
  };
  return rootNode.value ? walk(rootNode.value) : null;
}

async function refreshDir(path: string): Promise<void> {
  const node = findNode(path);
  if (node) await loadChildren(node);
}

// --- Context menu ---

function openMenu(ev: MouseEvent, node: TreeNode | null): void {
  const target = node ?? rootNode.value;
  if (!target) return;
  cancelEdit();
  opError.value = null;
  menu.value = { node: target, isRoot: !node, x: ev.clientX, y: ev.clientY };
}

const menuItems = computed(() =>
  menu.value ? explorerMenuItems({ isDir: menu.value.node.isDir, isRoot: menu.value.isRoot }) : [],
);

async function onMenuSelect(action: ExplorerAction): Promise<void> {
  const state = menu.value;
  menu.value = null;
  if (!state) return;
  const node = state.node;
  try {
    switch (action) {
      case "open":
        await activate(node);
        break;
      case "newFile":
        await beginCreate(node, false);
        break;
      case "newFolder":
        await beginCreate(node, true);
        break;
      case "rename":
        editing.value = { kind: "rename", path: node.path, name: node.name };
        editError.value = null;
        break;
      case "delete":
        await remove(node);
        break;
      case "copyPath":
        await navigator.clipboard.writeText(node.path);
        break;
      case "copyRelativePath":
        await navigator.clipboard.writeText(
          relativeDisplayPath(rootNode.value?.path ?? "", node.path),
        );
        break;
      case "reveal":
        await revealItemInDir(node.path);
        break;
      case "refresh":
        await loadChildren(node);
        break;
    }
  } catch (e) {
    opError.value = messageOf(e);
  }
}

// --- File operations ---

/**
 * Open the draft row for a new entry. New entries always land *inside* a
 * directory row and *beside* a file row, so the target directory is expanded and
 * loaded first — the draft renders as its first child.
 */
async function beginCreate(target: TreeNode, isDir: boolean): Promise<void> {
  const dir = target.isDir ? target : findParentOf(target.path) ?? rootNode.value;
  if (!dir) return;
  dir.expanded = true;
  if (!dir.loaded) await loadChildren(dir);
  editError.value = null;
  editing.value = { kind: "create", parent: dir.path, isDir, name: "" };
}

async function remove(node: TreeNode): Promise<void> {
  const ok = await confirm({
    message: node.isDir
      ? t('Delete folder "{name}" and everything inside it?', { name: node.name })
      : t('Delete "{name}"?', { name: node.name }),
    confirmLabel: t("Delete"),
  });
  if (!ok) return;
  await api.deletePath(node.path);
  const parent = findParentOf(node.path);
  if (parent) await loadChildren(parent);
}

function cancelEdit(): void {
  editing.value = null;
  editError.value = null;
}

async function commitEdit(): Promise<void> {
  const state = editing.value;
  if (!state || busy.value) return;
  const name = state.name.trim();
  // An untouched rename (and an empty draft) is a no-op, not an error.
  if (state.kind === "rename" && name === basename(state.path)) {
    cancelEdit();
    return;
  }
  if (state.kind === "create" && !name) {
    cancelEdit();
    return;
  }
  const invalid = validateEntryName(name);
  if (invalid) {
    editError.value = t(invalid);
    return;
  }
  busy.value = true;
  try {
    if (state.kind === "rename") {
      await api.renamePath(state.path, name);
      const parent = findParentOf(state.path);
      if (parent) await loadChildren(parent);
    } else {
      const created = await api.createEntry(state.parent, name, state.isDir);
      await refreshDir(state.parent);
      // A new file opens straight into the editor, like Quick Open's flow.
      if (!state.isDir) await resources.openFile(created);
    }
    cancelEdit();
  } catch (e) {
    // Keep the editor open with the typed name so the user can correct it.
    editError.value = messageOf(e);
  } finally {
    busy.value = false;
  }
}

// Focus the inline editor as it appears, preselecting the file stem (not the
// extension) the way a file manager does.
watch(editing, async (state) => {
  if (!state) return;
  await nextTick();
  const el = editInput.value;
  if (!el) return;
  el.focus();
  const dot = el.value.lastIndexOf(".");
  if (dot > 0) el.setSelectionRange(0, dot);
  else el.select();
});

// Flatten the expanded tree into visible rows with depth, for a simple list
// render (no per-level recursion in the template). A pending "new file/folder"
// draft is spliced in as the first child of its parent directory.
interface Row {
  node: TreeNode;
  depth: number;
  draft?: boolean;
}

const draftNode = computed<TreeNode | null>(() => {
  const state = editing.value;
  return state?.kind === "create" ? makeNode("", "__draft__", state.isDir) : null;
});

const rows = computed<Row[]>(() => {
  const out: Row[] = [];
  const draftParent = editing.value?.kind === "create" ? editing.value.parent : null;
  const walk = (nodes: TreeNode[], depth: number, parentPath: string) => {
    const draft = draftNode.value;
    if (draft && draftParent === parentPath) out.push({ node: draft, depth, draft: true });
    for (const node of nodes) {
      out.push({ node, depth });
      if (node.isDir && node.expanded) walk(node.children, depth + 1, node.path);
    }
  };
  if (rootNode.value) walk(rootNode.value.children, 0, rootNode.value.path);
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

      <div class="tree" @contextmenu.prevent="openMenu($event, null)">
        <template v-if="rootNode">
          <div v-if="rootNode.loading && !rootNode.loaded" class="hint"> {{ t("Loading…") }} </div>
          <div v-else-if="rootNode.error" class="hint error">{{ rootNode.error }}</div>
          <div v-else-if="!rows.length" class="hint">{{ t("Empty folder.") }}</div>
          <div
            v-for="{ node, depth, draft } in rows"
            :key="draft ? '__draft__' : node.path"
            :class="['row', { dir: node.isDir, hidden: node.hidden }]"
            :style="{ paddingLeft: 6 + depth * 12 + 'px' }"
            :title="draft ? undefined : node.name"
            :draggable="!node.isDir && !draft"
            @click="draft ? undefined : activate(node)"
            @contextmenu.prevent.stop="openMenu($event, draft ? null : node)"
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
            <input
              v-if="editing && (draft ? editing.kind === 'create' : editing.kind === 'rename' && editing.path === node.path)"
              :ref="(el) => { editInput = el as HTMLInputElement | null; }"
              v-model="editing.name"
              class="entry-edit"
              type="text"
              spellcheck="false"
              :disabled="busy"
              :placeholder="t(draft && node.isDir ? 'Folder name' : 'File name')"
              @click.stop
              @keydown.enter.prevent="commitEdit"
              @keydown.esc.prevent.stop="cancelEdit"
              @blur="commitEdit"
            />
            <span v-else class="entry-name">{{ node.name }}</span>
          </div>
          <div v-if="editError" class="hint error">{{ editError }}</div>
          <div v-if="opError" class="hint error">{{ opError }}</div>
        </template>
        <div v-else class="hint">{{ t("No folder yet. Focus a terminal and press") }} <button class="inline-sync" type="button" :disabled="!canSync" @click="syncToTerminal">{{ t("sync") }}</button> {{ t("to root the tree.") }}</div>
      </div>

      <ExplorerContextMenu
        v-if="menu"
        :items="menuItems"
        :name="menu.isRoot ? rootName : menu.node.name"
        :x="menu.x"
        :y="menu.y"
        @close="menu = null"
        @select="onMenuSelect"
      />
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
  height: 100%;
  min-width: 0;
  padding: 0 4px 8px 8px;
}
.strip {
  display: flex;
  flex-shrink: 0;
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
.tool:focus-visible { outline: 1px solid var(--accent); outline-offset: -2px; }
.tool .ico { font-size: 17px; }
.head {
  display: flex;
  flex-shrink: 0;
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
.sync:hover:not(:disabled) { color: var(--accent); background: #262626; }
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
.entry-edit {
  flex: 1;
  min-width: 0;
  height: 20px;
  padding: 0 4px;
  border: 1px solid var(--accent);
  border-radius: 3px;
  background: #101010;
  color: #e6e6e6;
  font: inherit;
  outline: none;
}
.entry-edit:disabled { opacity: 0.6; }
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
  color: var(--accent);
  cursor: pointer;
  padding: 0 2px;
  font: inherit;
  text-decoration: underline;
}
.inline-sync:disabled { color: #666; cursor: default; text-decoration: none; }
</style>
