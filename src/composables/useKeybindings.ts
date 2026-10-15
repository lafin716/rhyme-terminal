import { reactive, watch } from "vue";
import type { Keybinding, ActionId } from "../lib/keybindings";
import { ACTIONS, getAction } from "../lib/keybindings";
import { loadKeybindings, saveKeybindings } from "../lib/persistence";

interface State {
  overrides: Record<string, Keybinding>;
}

const state = reactive<State>({ overrides: {} });

let loaded = false;
let saveTimer: number | null = null;

function scheduleSave() {
  if (saveTimer !== null) window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    saveKeybindings({ ...state.overrides });
    saveTimer = null;
  }, 150);
}

watch(state, scheduleSave, { deep: true });

export function loadKeybindingsFromStorage() {
  if (loaded) return;
  loaded = true;
  const stored = loadKeybindings();
  if (stored) state.overrides = stored;
}

export function useKeybindings() {
  function bindingFor(id: ActionId): Keybinding {
    if (id in state.overrides) return state.overrides[id];
    return getAction(id)?.default ?? null;
  }

  function prefixFor(id: ActionId): string | null {
    const key = `${id}:prefix`;
    if (key in state.overrides) return state.overrides[key]?.key ?? null;
    return getAction(id)?.defaultPrefix ?? null;
  }

  function setPrefix(id: ActionId, key: string | null) {
    state.overrides[`${id}:prefix`] = key ? { key } : null;
  }

  function resetPrefix(id: ActionId) {
    delete state.overrides[`${id}:prefix`];
  }

  function setBinding(id: ActionId, b: Keybinding) {
    state.overrides[id] = b;
  }

  function resetBinding(id: ActionId) {
    delete state.overrides[id];
  }

  function resetAll() {
    for (const k of Object.keys(state.overrides)) delete state.overrides[k];
  }

  function isOverridden(id: ActionId): boolean {
    return id in state.overrides;
  }

  return {
    state,
    actions: ACTIONS,
    bindingFor,
    prefixFor,
    setPrefix,
    resetPrefix,
    setBinding,
    resetBinding,
    resetAll,
    isOverridden,
  };
}
