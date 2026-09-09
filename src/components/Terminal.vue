<script setup lang="ts">
import { t } from "../composables/useI18n";
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  Terminal,
  type ILink,
  type ILinkDecorations,
  type ILinkProvider,
} from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { CanvasAddon } from "@xterm/addon-canvas";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { ClipboardAddon } from "@xterm/addon-clipboard";
import "@xterm/xterm/css/xterm.css";
import {
  api,
  base64ToBytes,
  onPtyOutput,
  stringToBase64,
  type PtyOutputPayload,
} from "../lib/tauri";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { openPalette } from "../composables/usePalette";
import { useResources } from "../composables/useResources";
import { useSessions } from "../composables/useSessions";
import { FILE_DRAG_MIME, formatPathForInsertion } from "../lib/path-insert";
import { TERMINAL_BASE_FONT_SIZE, useTerminalZoom } from "../composables/useTerminalZoom";
import { useKeybindings } from "../composables/useKeybindings";
import TerminalLinkMenu from "./TerminalLinkMenu.vue";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { matchesEvent } from "../lib/keybindings";

const props = defineProps<{
  sessionId: string;
  active: boolean;
}>();

const linkMenu = ref<{ target: string; x: number; y: number; web: boolean } | null>(null);
const host = ref<HTMLDivElement | null>(null);
let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let webglAddon: WebglAddon | null = null;
let unlistenOutput: UnlistenFn | null = null;
let resizeObserver: ResizeObserver | null = null;
let isComposing = false;
let suppressUntil = 0;
let disposed = false;
const resources = useResources();
const sessions = useSessions();
const { bindingFor } = useKeybindings();
const { onZoomKeyDown, onZoomWheel, toastPercent } = useTerminalZoom((fontSize) => {
  if (!term) return;
  term.options.fontSize = fontSize;
  safeFit();
});

const FILE_LINK_RE = /(?:"[^"\r\n]+"|'[^'\r\n]+'|(?:[a-zA-Z]:[\\/]|\.{1,2}[\\/]|[\\/])[^ \t<>|?"'\r\n]+|(?:[\w.@()-]+[\\/])+[\w.@()-]+|[\w.@()-]+\.[a-zA-Z0-9]{1,10})(?::\d+){0,2}/g;

function detectWindowsBuild(): number {
  const uaData = (navigator as any).userAgentData;
  const ver = uaData?.platformVersion;
  if (typeof ver === "string") {
    const parts = ver.split(".").map((n: string) => parseInt(n, 10));
    if (parts.length >= 3 && Number.isFinite(parts[2])) return parts[2];
  }
  return 19045;
}

async function init() {
  if (!host.value) return;
  term = new Terminal({
    allowProposedApi: true,
    allowTransparency: false,
    fontFamily: '"Cascadia Mono", "Consolas", "Courier New", monospace',
    fontSize: TERMINAL_BASE_FONT_SIZE,
    fontWeight: "normal",
    fontWeightBold: "bold",
    lineHeight: 1.0,
    letterSpacing: 0,
    cursorBlink: true,
    cursorStyle: "block",
    cursorWidth: 1,
    scrollback: 10000,
    smoothScrollDuration: 125,
    minimumContrastRatio: 4.5,
    rescaleOverlappingGlyphs: true,
    wordSeparator: ' ()[]{}\',"`─',
    macOptionIsMeta: false,
    rightClickSelectsWord: false,
    windowsPty: { backend: "conpty", buildNumber: detectWindowsBuild() },
    linkHandler: {
      activate: (event, text) => {
        activateResource(event, text);
      },
    },
    theme: {
      background: "#1e1e1e",
      foreground: "#d4d4d4",
    },
  });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.loadAddon(new WebLinksAddon((event, uri) => {
    activateResource(event, uri);
  }));
  term.registerLinkProvider(createFileLinkProvider(term));
  term.parser.registerOscHandler(7, (data) => {
    const cwd = cwdFromOsc7(data);
    if (cwd) sessions.setCurrentCwd(props.sessionId, cwd);
    return true;
  });
  term.parser.registerOscHandler(9, (data) => {
    const cwd = data.startsWith("9;") ? data.slice(2) : "";
    if (cwd) sessions.setCurrentCwd(props.sessionId, cwd);
    return true;
  });
  const unicode11 = new Unicode11Addon();
  term.loadAddon(unicode11);
  term.unicode.activeVersion = "11";
  term.loadAddon(new ClipboardAddon());
  term.attachCustomKeyEventHandler(handleKeyEvent);
  term.open(host.value);

  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => {
      webgl.dispose();
      webglAddon = null;
      if (!disposed) term?.loadAddon(new CanvasAddon());
    });
    term.loadAddon(webgl);
    webglAddon = webgl;
  } catch {
    term.loadAddon(new CanvasAddon());
  }

  // Initial fit (after fonts settle so glyph metrics are stable)
  await nextRaf();
  if (disposed || !term) return;
  if (document.fonts?.ready) {
    try { await document.fonts.ready; } catch { /* ignore */ }
  }
  if (disposed || !term) return;
  safeFit();

  // Restore scrollback
  try {
    const b64 = await api.attachSession(props.sessionId);
    if (disposed || !term) return;
    if (b64) {
      const bytes = base64ToBytes(b64);
      term.write(bytes);
    }
  } catch (e) {
    console.error("attach failed", e);
  }
  if (disposed || !term) return;

  // Send initial resize to backend (in case fit changed dimensions)
  await api.resizeSession(props.sessionId, term.cols, term.rows).catch(() => {});
  if (disposed || !term) return;

  // IME composition tracking: WebView2 + xterm.js may deliver the composed
  // string via both compositionend and a follow-up onData, duplicating Hangul.
  const helper = host.value.querySelector<HTMLTextAreaElement>(".xterm-helper-textarea");
  if (helper) {
    helper.addEventListener("compositionstart", () => {
      isComposing = true;
    });
    helper.addEventListener("compositionend", (ev: CompositionEvent) => {
      isComposing = false;
      if (ev.data) {
        api.writeSession(props.sessionId, stringToBase64(ev.data)).catch((e) => {
          console.error("write failed", e);
        });
      }
      suppressUntil = performance.now() + 50;
    });
  }

  // User input → backend
  term.onData((data) => {
    if (isComposing) return;
    if (performance.now() < suppressUntil) return;
    api.writeSession(props.sessionId, stringToBase64(data)).catch((e) => {
      console.error("write failed", e);
    });
  });

  term.onResize(({ cols, rows }) => {
    api.resizeSession(props.sessionId, cols, rows).catch(() => {});
  });

  // Listen for output events filtered by id
  unlistenOutput = await onPtyOutput((payload: PtyOutputPayload) => {
    if (payload.id !== props.sessionId || !term) return;
    const bytes = base64ToBytes(payload.data);
    term.write(bytes);
  });
  if (disposed || !term) {
    unlistenOutput?.();
    unlistenOutput = null;
    return;
  }

  // Resize on container resize
  resizeObserver = new ResizeObserver(() => safeFit());
  resizeObserver.observe(host.value);

  host.value.addEventListener("mousedown", onHostMouseDown, { capture: true });
  host.value.addEventListener("auxclick", onHostAuxClick, { capture: true });
  host.value.addEventListener("contextmenu", onContextMenu);

  if (props.active) term.focus();
}

function createFileLinkProvider(terminal: Terminal): ILinkProvider {
  return {
    provideLinks(bufferLineNumber, callback) {
      const line = terminal.buffer.active.getLine(bufferLineNumber - 1);
      if (!line) {
        callback(undefined);
        return;
      }
      const text = line.translateToString(true);
      const links: ILink[] = [];
      FILE_LINK_RE.lastIndex = 0;
      for (const match of text.matchAll(FILE_LINK_RE)) {
        if (match.index === undefined) continue;
        const raw = match[0].trim();
        if (!raw || /^https?:\/\//i.test(raw)) continue;
        const decorations: ILinkDecorations = {
          pointerCursor: true,
          underline: true,
        };
        const link: ILink = {
          text: raw,
          range: {
            start: { x: match.index + 1, y: bufferLineNumber },
            end: { x: match.index + match[0].length, y: bufferLineNumber },
          },
          decorations,
          activate(event, target) {
            activateResource(event, target);
          },
        };
        links.push(link);
      }
      callback(links.length ? links : undefined);
    },
  };
}

function cwdFromOsc7(data: string): string | null {
  try {
    const url = new URL(data);
    if (url.protocol !== "file:") return null;
    let path = decodeURIComponent(url.pathname);
    if (url.hostname && url.hostname !== "localhost") {
      return `\\\\${url.hostname}${path.replace(/\//g, "\\")}`;
    }
    if (/^\/[a-zA-Z]:\//.test(path)) path = path.slice(1);
    return path.replace(/\//g, "\\");
  } catch {
    return null;
  }
}

function activateResource(event: MouseEvent, raw: string) {
  event.preventDefault();
  const text = raw.trim();
  const matches = (id: string) => {
    const b = bindingFor(id);
    return b?.key === "Click" && !!b.ctrl === event.ctrlKey && !!b.shift === event.shiftKey
      && !!b.alt === event.altKey && !!b.meta === event.metaKey;
  };
  if (matches("terminal.openLinkExternal")) { void openExternal(text).catch(showOpenError); return; }
  if (matches("terminal.openLink")) { openResource(text); return; }
  linkMenu.value = { target: text, x: event.clientX, y: event.clientY, web: /^https?:\/\//i.test(text) };
}

async function openExternal(text: string) {
  if (/^https?:\/\//i.test(text)) await openUrl(text);
  else await revealItemInDir(await api.resolveResourcePath(text, sessions.currentCwd(props.sessionId)));
}

function closeLinkMenu() { linkMenu.value = null; if (props.active) term?.focus(); }
function openMenuResource(external: boolean) {
  const text = linkMenu.value?.target;
  closeLinkMenu();
  if (!text) return;
  if (external) void openExternal(text).catch(showOpenError);
  else openResource(text);
}

function openResource(raw: string) {
  const text = raw.trim();
  if (/^https?:\/\//i.test(text)) {
    resources.openBrowser(text).catch(showOpenError);
    return;
  }
  resources.openFile(text, sessions.currentCwd(props.sessionId)).catch(showOpenError);
}

function showOpenError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  alert(t("Unable to open resource: {message}", { message }));
}

function handleKeyEvent(ev: KeyboardEvent): boolean {
  if (ev.type !== "keydown" || !term || ev.isComposing) return true;
  const matches = (id: string) => matchesEvent(bindingFor(id), ev);

  // Clipboard commands use the current settings, including alternate bindings.
  if (matches("terminal.copy")) {
    ev.preventDefault();
    copySelection();
    return false;
  }
  if (matches("terminal.paste") || matches("terminal.pasteAlternate")) {
    ev.preventDefault();
    pasteFromClipboard();
    return false;
  }

  // Copy the selection, or send the terminal interrupt byte when none is selected.
  if (matches("terminal.copyOrInterrupt")) {
    ev.preventDefault();
    if (term.hasSelection()) {
      copySelection();
    } else {
      api.writeSession(props.sessionId, stringToBase64("\x03")).catch(console.error);
    }
    return false;
  }

  // Do not let the browser's native paste bypass a remapped/cleared binding.
  if (ev.ctrlKey && !ev.altKey && ev.key.toLowerCase() === "v") {
    ev.preventDefault();
    return false;
  }

  return true;
}

function copySelection() {
  if (!term || !term.hasSelection()) return;
  const text = term.getSelection();
  if (!text) return;
  navigator.clipboard.writeText(text).catch((e) => console.error("copy failed", e));
  term.clearSelection();
}

function pasteFromClipboard() {
  if (!term) return;
  navigator.clipboard
    .readText()
    .then((text) => {
      if (text) term?.paste(text);
    })
    .catch((e) => console.error("paste failed", e));
}

function onContextMenu(ev: MouseEvent) {
  if (!term) return;
  ev.preventDefault();
  if (term.hasSelection()) {
    copySelection();
  } else {
    pasteFromClipboard();
  }
}

function onHostMouseDown(ev: MouseEvent) {
  if (ev.button !== 1) return;
  ev.preventDefault();
  ev.stopPropagation();
  openPalette(ev.clientX, ev.clientY, props.sessionId);
}

function onHostAuxClick(ev: MouseEvent) {
  if (ev.button !== 1) return;
  ev.preventDefault();
  ev.stopPropagation();
}

// A file dragged from the Explorer carries its absolute path on FILE_DRAG_MIME.
// Only allow the drop for that payload (preventDefault enables it) so unrelated
// drags — including Pane-tab drags (text/plain) — are left untouched.
function onHostDragOver(ev: DragEvent) {
  if (!ev.dataTransfer?.types.includes(FILE_DRAG_MIME)) return;
  ev.preventDefault();
  ev.dataTransfer.dropEffect = "copy";
}

function onHostDrop(ev: DragEvent) {
  const path = ev.dataTransfer?.getData(FILE_DRAG_MIME);
  if (!path) return; // directory / non-file / unrelated drag — ignore
  ev.preventDefault();
  // Write the (quoted-if-spaced) path to the PTY the same way typed/pasted text
  // is written. No newline: the path is inserted, not executed.
  api.writeSession(props.sessionId, stringToBase64(formatPathForInsertion(path)))
    .catch((e) => console.error("write failed", e));
  term?.focus();
}

function safeFit() {
  if (!fitAddon || !term || !host.value) return;
  if (host.value.offsetWidth === 0 || host.value.offsetHeight === 0) return;
  try {
    fitAddon.fit();
  } catch (e) {
    // ignore
  }
}

function nextRaf(): Promise<void> {
  return new Promise((res) => requestAnimationFrame(() => res()));
}

watch(
  () => props.active,
  async (isActive) => {
    if (!isActive) linkMenu.value = null;
    if (isActive) {
      await nextRaf();
      safeFit();
      term?.focus();
    }
  },
);

onMounted(init);

onBeforeUnmount(() => {
  disposed = true;
  try { unlistenOutput?.(); } catch { /* ignore */ }
  unlistenOutput = null;
  try { resizeObserver?.disconnect(); } catch { /* ignore */ }
  resizeObserver = null;
  if (host.value) {
    host.value.removeEventListener("mousedown", onHostMouseDown, { capture: true });
    host.value.removeEventListener("auxclick", onHostAuxClick, { capture: true });
    host.value.removeEventListener("contextmenu", onContextMenu);
  }
  // WebglAddon's internal cleanup can throw if the terminal core's _store is
  // already torn down; swallow so Vue's unmount cycle completes cleanly.
  try { webglAddon?.dispose(); } catch { /* ignore */ }
  webglAddon = null;
  try { term?.dispose(); } catch { /* ignore */ }
  term = null;
  fitAddon = null;
});

function resetTerminal() {
  if (!term) return;
  // Disable any leftover modes (mouse tracking, bracketed paste, alt screen),
  // restore cursor visibility and SGR — then reset xterm.js's parser state.
  term.write(
    "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?2004l\x1b[?1049l\x1b[?25h\x1b[0m",
  );
  term.reset();
  term.focus();
}

defineExpose({
  focus: () => term?.focus(),
  fit: safeFit,
  reset: resetTerminal,
});
</script>

<template>
  <TerminalLinkMenu v-if="linkMenu" :key="`${linkMenu.target}:${linkMenu.x}:${linkMenu.y}`" v-bind="linkMenu" @close="closeLinkMenu" @open="openMenuResource" />
  <div
    ref="host"
    class="term-host"
    @keydown.capture="onZoomKeyDown"
    @wheel.capture="onZoomWheel"
    @dragover="onHostDragOver"
    @drop="onHostDrop"
  />
  <Teleport to="body">
    <Transition name="terminal-zoom">
      <div v-if="toastPercent !== null" class="terminal-zoom-toast" role="status" aria-live="polite" aria-atomic="true">
        {{ toastPercent }}%
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.term-host {
  width: 100%;
  height: 100%;
  background: #1e1e1e;
  padding: 4px;
  box-sizing: border-box;
  overflow: hidden;
}

.terminal-zoom-toast {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  z-index: 10000;
  pointer-events: none;
  user-select: none;
  padding: 16px 28px;
  border: 1px solid rgb(255 255 255 / 14%);
  border-radius: 14px;
  background: rgb(30 30 30 / 78%);
  backdrop-filter: blur(10px);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
  color: #f5f5f5;
  font-size: 28px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  line-height: 1.2;
}

.terminal-zoom-enter-active,
.terminal-zoom-leave-active {
  transition: opacity 150ms ease;
}

.terminal-zoom-enter-from,
.terminal-zoom-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .terminal-zoom-enter-active,
  .terminal-zoom-leave-active {
    transition: none;
  }
}
</style>
