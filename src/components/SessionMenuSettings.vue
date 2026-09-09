<script setup lang="ts">
import { computed, ref } from "vue";
import { t } from "../composables/useI18n";
import { usePrefs } from "../composables/usePrefs";
import { moveSessionMenuItem, normalizeSessionMenuOrder, orderedSessionMenuItems } from "../lib/session-menu";

const { prefs, setPref } = usePrefs();
const items = computed(() => orderedSessionMenuItems(prefs.sessionMenuOrder));
const draggedId = ref<string | null>(null);
const dropTarget = ref<{ id: string; after: boolean } | null>(null);

function endDrag() {
  draggedId.value = null;
  dropTarget.value = null;
}

function startDrag(event: DragEvent, id: string) {
  if (!event.dataTransfer) return;
  draggedId.value = id;
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData("text/plain", id);
  const row = (event.currentTarget as HTMLElement).closest("li");
  if (row) event.dataTransfer.setDragImage(row, 12, row.clientHeight / 2);
}

function dragOver(event: DragEvent, id: string) {
  if (!draggedId.value) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
  dropTarget.value = id === draggedId.value ? null : {
    id, after: event.clientY > bounds.top + bounds.height / 2,
  };
}

function drop(event: DragEvent, id: string) {
  if (!draggedId.value) return;
  event.preventDefault();
  event.stopPropagation();
  dragOver(event, id);
  if (dropTarget.value) {
    const order = normalizeSessionMenuOrder(prefs.sessionMenuOrder).filter((item) => item !== draggedId.value);
    const index = order.indexOf(dropTarget.value.id) + (dropTarget.value.after ? 1 : 0);
    order.splice(index, 0, draggedId.value);
    setPref("sessionMenuOrder", order);
  }
  endDrag();
}
function move(id: string, offset: -1 | 1) {
  setPref("sessionMenuOrder", moveSessionMenuItem(prefs.sessionMenuOrder, id, offset));
}
</script>

<template>
  <section class="session-menu-settings" aria-labelledby="session-menu-title">
    <div class="heading">
      <h3 id="session-menu-title">{{ t('Session tab options menu order') }}</h3>
      <button type="button" @click="setPref('sessionMenuOrder', normalizeSessionMenuOrder(undefined))">{{ t('Restore default order') }}</button>
    </div>
    <p>{{ t('Drag the handles or use the up and down buttons to reorder items. Changes are saved automatically and apply to all session tabs.') }}</p>
    <ol @dragleave="dropTarget = null">
      <li v-for="(item, index) in items" :key="item.id" :data-menu-item="item.id"
        :class="{ dragging: draggedId === item.id, 'drop-before': dropTarget?.id === item.id && !dropTarget.after, 'drop-after': dropTarget?.id === item.id && dropTarget.after }"
        @dragover.stop="dragOver($event, item.id)" @drop="drop($event, item.id)">
        <span class="drag-handle" draggable="true" :title="t('Drag to reorder')" :aria-label="t('Drag to reorder')"
          @dragstart.stop="startDrag($event, item.id)" @dragend.stop="endDrag">
          <svg width="16" height="20" viewBox="0 0 16 20" fill="currentColor" aria-hidden="true">
            <circle cx="5" cy="5" r="1.5" /><circle cx="11" cy="5" r="1.5" />
            <circle cx="5" cy="10" r="1.5" /><circle cx="11" cy="10" r="1.5" />
            <circle cx="5" cy="15" r="1.5" /><circle cx="11" cy="15" r="1.5" />
          </svg>
        </span>
        <span class="label">{{ t(item.label) }}</span>
        <button type="button" :disabled="index === 0" :aria-label="t('Move {item} up', { item: t(item.label) })" @click="move(item.id, -1)">↑</button>
        <button type="button" :disabled="index === items.length - 1" :aria-label="t('Move {item} down', { item: t(item.label) })" @click="move(item.id, 1)">↓</button>
      </li>
    </ol>
    <p>{{ t('The shell matching the default terminal is hidden in the options menu.') }}</p>
  </section>
</template>

<style scoped>
.session-menu-settings { margin-bottom: 24px; padding: 20px; border: 1px solid #333; border-radius: 8px; }
.heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
h3 { margin: 0; font-size: 14px; color: #e6e6e6; }
p { color: #999; font-size: 12px; line-height: 1.6; }
ol { padding: 0; margin: 16px 0; list-style: none; }
li { position: relative; display: flex; align-items: center; gap: 8px; padding: 7px 0; border-bottom: 1px solid #2b2b2b; }
li.dragging { opacity: .45; }
li.drop-before::before, li.drop-after::after { content: ""; position: absolute; left: 0; right: 0; height: 2px; background: #4ec9b0; pointer-events: none; }
li.drop-before::before { top: -1px; }
li.drop-after::after { bottom: -1px; }
.drag-handle { display: flex; align-items: center; justify-content: center; flex: 0 0 24px; height: 30px; color: #888; cursor: grab; user-select: none; }
.drag-handle:hover { color: #ddd; }
.drag-handle:active { cursor: grabbing; }
.drag-handle svg { pointer-events: none; }
.label { flex: 1; font-size: 13px; }
button { min-width: 30px; min-height: 30px; padding: 4px 10px; border: 1px solid #444; border-radius: 4px; background: #292929; color: #ddd; cursor: pointer; }
button:hover:enabled { background: #383838; }
button:disabled { opacity: .3; cursor: default; }
button:focus-visible { outline: 2px solid #4ec9b0; outline-offset: 2px; }
</style>
