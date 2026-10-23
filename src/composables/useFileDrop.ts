import { t } from "./useI18n";
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { api, onExternalFileDrop } from "../lib/tauri";
import {
  DROP_MESSAGE_PREFIX,
  isExternalFileDrag,
  matchDroppedPaths,
  previewFromFile,
} from "../lib/file-drop";
import { useResources } from "./useResources";
import { useFocus } from "./useFocus";

/** WebView2 exposes this on Windows; it hands `File` objects to the host. */
interface WebView2Bridge {
  postMessageWithAdditionalObjects(message: string, objects: ArrayLike<File>): void;
}

function webView2(): WebView2Bridge | undefined {
  const bridge = (window as unknown as { chrome?: { webview?: Partial<WebView2Bridge> } }).chrome?.webview;
  return typeof bridge?.postMessageWithAdditionalObjects === "function"
    ? (bridge as WebView2Bridge)
    : undefined;
}

/** How long to wait for the Windows host to send back the dropped paths. */
const PATH_TIMEOUT_MS = 3000;
/** dragleave is unreliable across child elements, so the overlay times out instead. */
const HOVER_TIMEOUT_MS = 150;

export interface DropHover {
  /** Pane the drop would open in, as a viewport rect; null outside a pane. */
  rect: { left: number; top: number; width: number; height: number } | null;
}

/**
 * Opens files dragged in from the OS file manager as tabs in the pane they are
 * dropped on. Listeners run in the capture phase so in-page `.stop` handlers
 * cannot hide the drag, and every external file drag is default-prevented so
 * the webview never navigates to the dropped file.
 */
export function useFileDrop() {
  const resources = useResources();
  const { setFocusedLeaf } = useFocus();
  const hover = ref<DropHover | null>(null);

  // A drag that started inside the page (tab, Explorer row, sidebar item).
  let internalDrag = false;
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  let nextId = 0;
  const pending = new Map<string, (paths: string[]) => void>();
  let unlisten: UnlistenFn | undefined;

  function leafElement(target: EventTarget | null): HTMLElement | null {
    return target instanceof Element ? target.closest<HTMLElement>("[data-leaf-id]") : null;
  }

  function clearHover() {
    clearTimeout(hoverTimer);
    hover.value = null;
  }

  function onDragStart() {
    internalDrag = true;
  }

  // dragend can be lost when the dragged element is removed mid-drag (a tab
  // moving panes), so any mouse movement after the drag also clears the flag.
  function onDragEnd() {
    internalDrag = false;
  }

  function onDragOver(ev: DragEvent) {
    if (!isExternalFileDrag(ev.dataTransfer?.types, internalDrag)) return;
    ev.preventDefault();
    if (ev.dataTransfer) ev.dataTransfer.dropEffect = "copy";
    const rect = leafElement(ev.target)?.getBoundingClientRect();
    hover.value = {
      rect: rect ? { left: rect.left, top: rect.top, width: rect.width, height: rect.height } : null,
    };
    clearTimeout(hoverTimer);
    hoverTimer = setTimeout(clearHover, HOVER_TIMEOUT_MS);
  }

  /** Real paths for the dropped files, or null when they cannot be recovered. */
  async function recoverPaths(files: File[]): Promise<string[] | null> {
    const names = files.map((file) => file.name);
    const bridge = webView2();
    if (bridge) {
      const id = String(++nextId);
      const paths = new Promise<string[]>((resolve) => {
        pending.set(id, resolve);
        setTimeout(() => resolve([]), PATH_TIMEOUT_MS);
      });
      try {
        bridge.postMessageWithAdditionalObjects(`${DROP_MESSAGE_PREFIX}${id}`, files);
        return matchDroppedPaths(names, await paths);
      } catch (e) {
        console.warn("Could not recover dropped file paths:", e);
        return null;
      } finally {
        pending.delete(id);
      }
    }
    try {
      return matchDroppedPaths(names, await api.droppedFilePaths());
    } catch (e) {
      console.warn("Could not recover dropped file paths:", e);
      return null;
    }
  }

  async function openDropped(files: File[], leafId: string | undefined) {
    const paths = await recoverPaths(files);
    const errors: string[] = [];
    for (const [index, file] of files.entries()) {
      // Each open goes to the drop pane; opening may move focus elsewhere
      // (an already-open file is activated where it lives).
      if (leafId) setFocusedLeaf(leafId);
      try {
        if (paths) await resources.openFile(paths[index]);
        else resources.openPreview(await previewFromFile(file));
      } catch (e) {
        errors.push(`${file.name}: ${e instanceof Error ? e.message : String(e)}`);
      }
    }
    if (errors.length) {
      alert(t("Unable to open resource: {message}", { message: errors.join("\n") }));
    }
  }

  function onDrop(ev: DragEvent) {
    const external = isExternalFileDrag(ev.dataTransfer?.types, internalDrag);
    internalDrag = false;
    if (!external) return;
    ev.preventDefault();
    clearHover();
    // Read the FileList synchronously; the DataTransfer is emptied after dispatch.
    const files = Array.from(ev.dataTransfer?.files ?? []);
    if (!files.length) return;
    const leafId = leafElement(ev.target)?.dataset.leafId;
    void openDropped(files, leafId);
  }

  onMounted(async () => {
    window.addEventListener("dragstart", onDragStart, true);
    window.addEventListener("dragend", onDragEnd, true);
    window.addEventListener("mousemove", onDragEnd, true);
    window.addEventListener("dragover", onDragOver, true);
    window.addEventListener("drop", onDrop, true);
    if (webView2()) {
      unlisten = await onExternalFileDrop(({ id, paths }) => pending.get(id)?.(paths));
    }
  });

  onBeforeUnmount(() => {
    window.removeEventListener("dragstart", onDragStart, true);
    window.removeEventListener("dragend", onDragEnd, true);
    window.removeEventListener("mousemove", onDragEnd, true);
    window.removeEventListener("dragover", onDragOver, true);
    window.removeEventListener("drop", onDrop, true);
    clearHover();
    unlisten?.();
  });

  return { hover };
}
