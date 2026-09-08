import { t } from "./useI18n";
import { reactive } from "vue";
import { api, type FilePreview } from "../lib/tauri";
import { nodeId } from "../lib/layout-types";
import { useWorkspaces } from "./useWorkspaces";
import { useFocus } from "./useFocus";
import {
  addTabToLeaf,
  findFirstLeaf,
  findLeafById,
  findLeafBySession,
  removeTab,
} from "./useLayout";

export interface FileTab {
  kind: "file";
  id: string;
  preview: FilePreview;
  /** True while the editor holds unsaved changes; drives the tab dirty dot. */
  dirty?: boolean;
}

export interface BrowserTab {
  kind: "browser";
  id: string;
  url: string;
  webviewLabel: string;
}

export type ResourceTab = FileTab | BrowserTab;

const state = reactive({
  tabs: {} as Record<string, ResourceTab>,
});

function normalizeUrl(raw: string): string {
  const trimmed = raw.trim();
  const url = new URL(trimmed);
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error(t("Only HTTP and HTTPS URLs can be opened inside rhyme-terminal."));
  }
  url.hash = url.hash;
  return url.toString();
}

export function isResourceTabId(id: string): boolean {
  return id.startsWith("resource-");
}

export function useResources() {
  const { activeWorkspace, replaceLayout } = useWorkspaces();
  const { focusedLeafId, setFocusedLeaf } = useFocus();

  function getById(id: string): ResourceTab | undefined {
    return state.tabs[id];
  }

  function targetLeafId(): string | null {
    const ws = activeWorkspace.value;
    if (!ws) return null;
    if (focusedLeafId.value && findLeafById(ws.layout, focusedLeafId.value)) {
      return focusedLeafId.value;
    }
    return findFirstLeaf(ws.layout).id;
  }

  function activateExisting(predicate: (tab: ResourceTab) => boolean): boolean {
    const ws = activeWorkspace.value;
    if (!ws) return false;
    for (const tab of Object.values(state.tabs)) {
      if (!predicate(tab)) continue;
      const leaf = findLeafBySession(ws.layout, tab.id);
      if (!leaf) continue;
      leaf.activeTabId = tab.id;
      setFocusedLeaf(leaf.id);
      return true;
    }
    return false;
  }

  async function openFile(target: string, cwd?: string): Promise<void> {
    const preview = await api.readFilePreview(target, cwd);
    const key = preview.canonicalPath.toLocaleLowerCase();
    if (
      activateExisting(
        (tab) => tab.kind === "file"
          && tab.preview.canonicalPath.toLocaleLowerCase() === key,
      )
    ) {
      return;
    }
    const leafId = targetLeafId();
    const ws = activeWorkspace.value;
    if (!leafId || !ws) return;
    const id = nodeId("resource-file");
    state.tabs[id] = { kind: "file", id, preview };
    addTabToLeaf(ws.layout, leafId, id);
    setFocusedLeaf(leafId);
  }

  async function openBrowser(rawUrl: string): Promise<void> {
    const url = normalizeUrl(rawUrl);
    if (activateExisting((tab) => tab.kind === "browser" && tab.url === url)) return;
    const leafId = targetLeafId();
    const ws = activeWorkspace.value;
    if (!leafId || !ws) return;
    const id = nodeId("resource-browser");
    state.tabs[id] = {
      kind: "browser",
      id,
      url,
      webviewLabel: `browser-${id.replace(/[^a-zA-Z0-9-]/g, "-")}`,
    };
    addTabToLeaf(ws.layout, leafId, id);
    setFocusedLeaf(leafId);
  }

  function closeResource(id: string): void {
    const ws = activeWorkspace.value;
    if (ws) {
      const { root } = removeTab(ws.layout, id);
      if (root !== ws.layout) replaceLayout(ws.id, root);
    }
    delete state.tabs[id];
  }

  function updateBrowserUrl(id: string, url: string): void {
    const tab = state.tabs[id];
    if (tab?.kind === "browser") tab.url = normalizeUrl(url);
  }

  function setFileDirty(id: string, dirty: boolean): void {
    const tab = state.tabs[id];
    if (tab?.kind === "file") tab.dirty = dirty;
  }

  // After a successful save, adopt the written text as the tab's stored content
  // so a re-focused/re-opened tab shows what was saved, and clear dirty.
  function updateFilePreviewText(id: string, text: string): void {
    const tab = state.tabs[id];
    if (tab?.kind !== "file") return;
    tab.preview.text = text;
    tab.dirty = false;
  }

  function forgetResource(id: string): void {
    delete state.tabs[id];
  }

  return {
    state,
    getById,
    openFile,
    openBrowser,
    closeResource,
    forgetResource,
    updateBrowserUrl,
    setFileDirty,
    updateFilePreviewText,
  };
}
