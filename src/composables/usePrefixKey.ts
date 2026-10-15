import { onMounted, onUnmounted } from "vue";

import { useFlowPage } from "./useFlowPage";
import { useKeybindings } from "./useKeybindings";
import { matchesEvent } from "../lib/keybindings";
import { useSettings } from "./useSettings";
import { confirmState } from "./useConfirm";
import { quickOpenState } from "./useQuickOpen";

type Handler = (key: string, ev: KeyboardEvent) => void;
export const prefixState = { armed: false };

/**
 * tmux-style prefix key handler. Defaults: prefix is Ctrl+B.
 * After prefix is pressed, the next key (within timeout) is captured and forwarded to handler.
 */
export function usePrefixKey(handler: Handler, opts: { timeoutMs?: number } = {}) {
  const { flowOpen } = useFlowPage();
  const { settingsOpen } = useSettings();
  const { bindingFor } = useKeybindings();
  const timeoutMs = opts.timeoutMs ?? 1500;
  let armed = false;
  let timer: number | null = null;

  function disarm() {
    armed = false;
    prefixState.armed = false;
    if (timer !== null) {
      window.clearTimeout(timer);
      timer = null;
    }
  }

  function onKeyDown(ev: KeyboardEvent) {
    if (flowOpen.value || settingsOpen.value || confirmState.open || quickOpenState.open || ev.isComposing) {
      disarm();
      return;
    }
    if (ev.defaultPrevented) return;
    if (armed) {
      ev.preventDefault();
      ev.stopPropagation();
      // ignore standalone modifier keys
      if (["Control", "Shift", "Alt", "Meta"].includes(ev.key)) return;
      const key = ev.key;
      disarm();
      handler(key, ev);
      return;
    }
    // Use the configurable prefix starter (Ctrl+B by default).
    if (matchesEvent(bindingFor("prefix.activate"), ev)) {
      ev.preventDefault();
      ev.stopPropagation();
      armed = true;
      prefixState.armed = true;
      timer = window.setTimeout(disarm, timeoutMs);
    }
  }

  onMounted(() => {
    window.addEventListener("keydown", onKeyDown, true);
  });
  onUnmounted(() => {
    window.removeEventListener("keydown", onKeyDown, true);
    disarm();
  });

  return {
    isArmed: () => armed,
  };
}
