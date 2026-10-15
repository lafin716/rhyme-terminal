<script setup lang="ts">
// The Explorer's right-click menu. Purely presentational: the item list comes
// from `explorer-menu.ts` and the panel emits the chosen action back to
// `ExplorerPanel.vue`. Positioning/dismissal follow `TerminalLinkMenu.vue` —
// clamp inside the viewport, close on outside pointerdown, Esc, or resize, and
// cycle focus with Tab/arrows.
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { t } from "../composables/useI18n";
import type { ExplorerAction, ExplorerMenuItem } from "../lib/explorer-menu";

const props = defineProps<{
  items: ExplorerMenuItem[];
  name: string;
  x: number;
  y: number;
}>();
const emit = defineEmits<{ close: []; select: [action: ExplorerAction] }>();

const panel = ref<HTMLElement>();
const left = ref(props.x);
const top = ref(props.y);

function close() {
  emit("close");
}

function outside(e: PointerEvent) {
  if (!panel.value?.contains(e.target as Node)) close();
}

function key(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopImmediatePropagation();
    close();
    return;
  }
  if (e.key !== "Tab" && e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  e.preventDefault();
  e.stopImmediatePropagation();
  const buttons = Array.from(panel.value?.querySelectorAll<HTMLButtonElement>("button") ?? []);
  const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
  const step = e.shiftKey || e.key === "ArrowUp" ? -1 : 1;
  buttons[(index + step + buttons.length) % buttons.length]?.focus();
}

onMounted(async () => {
  await nextTick();
  const rect = panel.value!.getBoundingClientRect();
  left.value = Math.max(8, Math.min(props.x, window.innerWidth - rect.width - 8));
  // Drop below the cursor when it fits, otherwise flip above it.
  top.value = Math.max(
    8,
    props.y + rect.height + 8 <= window.innerHeight ? props.y + 4 : props.y - rect.height - 4,
  );
  panel.value?.querySelector<HTMLButtonElement>("button")?.focus();
  window.addEventListener("pointerdown", outside, true);
  window.addEventListener("keydown", key, true);
  window.addEventListener("resize", close);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", outside, true);
  window.removeEventListener("keydown", key, true);
  window.removeEventListener("resize", close);
});
</script>

<template>
  <Teleport to="body">
    <div
      ref="panel"
      class="entry-menu"
      role="menu"
      :aria-label="t('File actions')"
      :style="{ left: `${left}px`, top: `${top}px` }"
      @pointerdown.stop
      @click.stop
      @contextmenu.prevent.stop
    >
      <div class="heading" :title="name">{{ name }}</div>
      <button
        v-for="item in items"
        :key="item.action"
        type="button"
        role="menuitem"
        :class="['action', { divider: item.divider, danger: item.danger }]"
        @click="emit('select', item.action)"
      >
        {{ t(item.label) }}
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.entry-menu {
  position: fixed;
  z-index: 10020;
  min-width: 194px;
  max-width: calc(100vw - 16px);
  padding: 4px;
  background: #141414;
  color: #e5e5e5;
  border: 1px solid #414141;
  border-radius: 7px;
  box-shadow: 0 8px 24px #0006;
  font-size: 12px;
}
.heading {
  padding: 4px 7px 7px;
  margin-bottom: 4px;
  border-bottom: 1px solid #ffffff12;
  color: #aaa;
  font: 11px/1.4 Consolas, monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.action {
  display: block;
  width: 100%;
  padding: 6px 7px;
  border: 0;
  border-radius: 4px;
  background: none;
  color: inherit;
  cursor: pointer;
  font: inherit;
  text-align: left;
}
.action.divider {
  margin-top: 5px;
  border-top: 1px solid #ffffff12;
  padding-top: 9px;
  border-radius: 0 0 4px 4px;
}
.action.danger { color: #f48771; }
.action:hover,
.action:focus-visible {
  background: #ffffff12;
  outline: none;
}
.action:focus-visible { box-shadow: inset 0 0 0 1px #888; }
</style>
