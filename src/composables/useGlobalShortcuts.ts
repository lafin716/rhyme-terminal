import { onMounted, onUnmounted } from "vue";
import { ACTIONS, matchesEvent, type ActionId } from "../lib/keybindings";
import { useKeybindings } from "./useKeybindings";
import { useFlowPage } from "./useFlowPage";
import { useSettings } from "./useSettings";
import { confirmState } from "./useConfirm";
import { quickOpenState } from "./useQuickOpen";
import { prefixState } from "./usePrefixKey";

type Handler = () => void | Promise<void>;

const handlers = new Map<string, Handler>();
let installed = false;

export function registerAction(id: ActionId, h: Handler) {
  handlers.set(id, h);
}

export function registerFocusSessionByIndex(fn: (i: number) => void) {
  for (let i = 0; i < 15; i++) registerAction(`session.focusIndex${i + 1}`, () => fn(i));
}

export function unregisterAction(id: ActionId) {
  handlers.delete(id);
}

export function dispatchAction(id: ActionId) {
  const h = handlers.get(id);
  if (h) void h();
}

function shouldIgnoreTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable) return true;
  return false;
}

export function useGlobalShortcuts() {
  const { bindingFor } = useKeybindings();
  const { settingsOpen } = useSettings();
  const { flowOpen } = useFlowPage();

  function onKeyDown(ev: KeyboardEvent) {
    if (ev.defaultPrevented || ev.isComposing || prefixState.armed) return;
    // Settings modal owns the keyboard while open (for capture mode and Esc-to-close).
    if (settingsOpen.value || flowOpen.value) return;
    // Confirm modal also owns the keyboard while open (Esc/Enter, prevent stacking).
    if (confirmState.open) return;
    // Quick Open owns the keyboard while open (typing, ↑/↓/Enter/Esc); its own
    // handler drives navigation, so suspend all other shortcuts underneath it.
    if (quickOpenState.open) return;

    // Never swallow plain typing in inputs/textareas. Modifier-bearing combos still pass.
    const hasMod = ev.ctrlKey || ev.metaKey || ev.altKey;
    if (!hasMod && ev.key.length === 1 && shouldIgnoreTarget(ev.target)) return;

    for (const a of ACTIONS) {
      if (a.scope) continue;
      const b = bindingFor(a.id as ActionId);
      if (matchesEvent(b, ev)) {
        ev.preventDefault();
        ev.stopPropagation();
        dispatchAction(a.id as ActionId);
        return;
      }
    }
  }

  onMounted(() => {
    if (installed) return;
    installed = true;
    window.addEventListener("keydown", onKeyDown, true);
  });
  onUnmounted(() => {
    window.removeEventListener("keydown", onKeyDown, true);
    installed = false;
  });
}
