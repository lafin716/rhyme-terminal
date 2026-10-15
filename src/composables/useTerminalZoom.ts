import { computed, onScopeDispose, ref, shallowRef } from "vue";
import { useKeybindings } from "./useKeybindings";
import { matchesEvent, matchesWheelEvent } from "../lib/keybindings";

export const TERMINAL_BASE_FONT_SIZE = 13;
const toast = shallowRef<{ owner: symbol; percent: number } | null>(null);

/** Zoom belongs to each terminal; only the latest change owns the center toast. */
export function useTerminalZoom(applyFontSize: (fontSize: number) => void) {
  const owner = Symbol("terminal-zoom");
  const { bindingFor } = useKeybindings();
  const percent = ref(100);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function changeZoom(direction: number) {
    const next = Math.min(300, Math.max(50, percent.value + direction * 10));
    if (next === percent.value) return;
    percent.value = next;
    applyFontSize(TERMINAL_BASE_FONT_SIZE * next / 100);
    toast.value = { owner, percent: next };
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (toast.value?.owner === owner) toast.value = null;
    }, 1100);
  }

  function consume(ev: Event) {
    // Prevent WebView page zoom and keep the gesture out of the PTY/xterm scrollback.
    ev.preventDefault();
    ev.stopPropagation();
  }

  function onZoomKeyDown(ev: KeyboardEvent) {
    if (ev.defaultPrevented || ev.isComposing) return;
    let direction = 0;
    if (["terminal.zoomIn", "terminal.zoomInWheel"].some(id => matchesEvent(bindingFor(id), ev))) direction = 1;
    else if (["terminal.zoomOut", "terminal.zoomOutWheel"].some(id => matchesEvent(bindingFor(id), ev))) direction = -1;
    if (!direction) return;
    consume(ev);
    changeZoom(direction);
  }

  function onZoomWheel(ev: WheelEvent) {
    let direction = 0;
    if (["terminal.zoomIn", "terminal.zoomInWheel"].some(id => matchesWheelEvent(bindingFor(id), ev))) direction = 1;
    else if (["terminal.zoomOut", "terminal.zoomOutWheel"].some(id => matchesWheelEvent(bindingFor(id), ev))) direction = -1;
    if (!direction) return;
    consume(ev);
    changeZoom(direction);
  }

  onScopeDispose(() => {
    clearTimeout(timer);
    if (toast.value?.owner === owner) toast.value = null;
  });

  return {
    onZoomKeyDown,
    onZoomWheel,
    toastPercent: computed(() => toast.value?.owner === owner ? toast.value.percent : null),
  };
}
