<script setup lang="ts">
import { t } from "../composables/useI18n";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  clampOffset,
  clampScale,
  fitScale,
  normalizeRotation,
  stepScale,
  zoomAround,
  type Point,
  type Size,
} from "../lib/image-view";

const props = defineProps<{ src: string; name: string }>();

const viewport = ref<HTMLDivElement | null>(null);
const natural = ref<Size>({ width: 0, height: 0 });
const view = ref<Size>({ width: 0, height: 0 });
const scale = ref(1);
const rotation = ref(0);
const offset = ref<Point>({ x: 0, y: 0 });
// While true the image follows the viewport size; any manual zoom drops it.
const fitted = ref(true);
const loaded = ref(false);
const failed = ref(false);

const percent = computed(() => `${Math.round(scale.value * 100)}%`);
const imageStyle = computed(() => ({
  width: `${natural.value.width}px`,
  height: `${natural.value.height}px`,
  transform:
    `translate(-50%, -50%) translate(${offset.value.x}px, ${offset.value.y}px) ` +
    `rotate(${rotation.value}deg) scale(${scale.value})`,
  // Pixel-art and screenshots read better unsmoothed when zoomed in.
  imageRendering: scale.value >= 2 ? ("pixelated" as const) : ("auto" as const),
}));

function setOffset(next: Point) {
  offset.value = clampOffset(next, natural.value, view.value, scale.value, rotation.value);
}

function fit() {
  fitted.value = true;
  scale.value = fitScale(natural.value, view.value, rotation.value);
  offset.value = { x: 0, y: 0 };
}

/** Zoom to `next`, keeping the image point under `anchor` (viewport-centred) still. */
function zoomTo(next: number, anchor: Point = { x: 0, y: 0 }) {
  const to = clampScale(next);
  if (to === scale.value) return;
  const moved = zoomAround(offset.value, anchor, scale.value, to);
  fitted.value = false;
  scale.value = to;
  setOffset(moved);
}

function actualSize() {
  zoomTo(1);
  if (scale.value === 1) fitted.value = false;
}

function rotate() {
  rotation.value = normalizeRotation(rotation.value + 90);
  if (fitted.value) fit();
  else setOffset(offset.value);
}

function anchorFor(ev: { clientX: number; clientY: number }): Point {
  const rect = viewport.value?.getBoundingClientRect();
  if (!rect) return { x: 0, y: 0 };
  return {
    x: ev.clientX - rect.left - rect.width / 2,
    y: ev.clientY - rect.top - rect.height / 2,
  };
}

function onWheel(ev: WheelEvent) {
  if (!loaded.value) return;
  ev.preventDefault();
  // Smooth, proportional zoom: trackpads send many small deltas, wheels few large ones.
  const factor = Math.exp(-ev.deltaY * (ev.deltaMode === 1 ? 0.05 : 0.002));
  zoomTo(scale.value * factor, anchorFor(ev));
}

let dragStart: { pointer: Point; offset: Point } | null = null;
const dragging = ref(false);

function onPointerDown(ev: PointerEvent) {
  if (ev.button !== 0 || !loaded.value) return;
  viewport.value?.focus();
  dragStart = { pointer: { x: ev.clientX, y: ev.clientY }, offset: { ...offset.value } };
  dragging.value = true;
  (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
}

function onPointerMove(ev: PointerEvent) {
  if (!dragStart) return;
  setOffset({
    x: dragStart.offset.x + ev.clientX - dragStart.pointer.x,
    y: dragStart.offset.y + ev.clientY - dragStart.pointer.y,
  });
}

function onPointerUp() {
  dragStart = null;
  dragging.value = false;
}

function onDoubleClick(ev: MouseEvent) {
  if (!loaded.value) return;
  if (fitted.value) zoomTo(1, anchorFor(ev));
  else fit();
}

function onKeydown(ev: KeyboardEvent) {
  if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing || !loaded.value) return;
  const handled = (() => {
    switch (ev.key) {
      case "+":
      case "=":
        zoomTo(stepScale(scale.value, 1));
        return true;
      case "-":
      case "_":
        zoomTo(stepScale(scale.value, -1));
        return true;
      case "0":
        fit();
        return true;
      case "1":
        actualSize();
        return true;
      case "r":
      case "R":
        rotate();
        return true;
      default:
        return false;
    }
  })();
  if (handled) {
    ev.preventDefault();
    ev.stopPropagation();
  }
}

function onLoad(ev: Event) {
  const img = ev.target as HTMLImageElement;
  natural.value = { width: img.naturalWidth, height: img.naturalHeight };
  loaded.value = true;
  failed.value = false;
  fit();
}

function onError() {
  loaded.value = false;
  failed.value = true;
}

let observer: ResizeObserver | null = null;
onMounted(() => {
  if (!viewport.value) return;
  observer = new ResizeObserver(([entry]) => {
    view.value = { width: entry.contentRect.width, height: entry.contentRect.height };
    if (fitted.value) fit();
    else setOffset(offset.value);
  });
  observer.observe(viewport.value);
});
onBeforeUnmount(() => observer?.disconnect());

watch(
  () => props.src,
  () => {
    loaded.value = false;
    failed.value = false;
    rotation.value = 0;
    fitted.value = true;
  },
);
</script>

<template>
  <div
    ref="viewport"
    class="image-viewer"
    :class="{ dragging }"
    tabindex="0"
    @wheel="onWheel"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
    @dblclick="onDoubleClick"
    @keydown="onKeydown"
  >
    <img
      v-show="loaded"
      :src="src"
      :alt="name"
      :style="imageStyle"
      draggable="false"
      @load="onLoad"
      @error="onError"
    />
    <div v-if="failed" class="message">{{ t("This image could not be displayed.") }}</div>

    <div v-if="loaded" class="controls" @pointerdown.stop @dblclick.stop>
      <button :title="t('Zoom out') + ' (-)'" @click="zoomTo(stepScale(scale, -1))">−</button>
      <span class="percent">{{ percent }}</span>
      <button :title="t('Zoom in') + ' (+)'" @click="zoomTo(stepScale(scale, 1))">+</button>
      <span class="sep" />
      <button :class="{ active: fitted }" :title="t('Fit to window') + ' (0)'" @click="fit">{{ t("Fit") }}</button>
      <button :title="t('Actual size') + ' (1)'" @click="actualSize">1:1</button>
      <button :title="t('Rotate') + ' (R)'" @click="rotate">⟳</button>
      <span class="sep" />
      <span class="dims">{{ natural.width }} × {{ natural.height }}</span>
    </div>
  </div>
</template>

<style scoped>
.image-viewer {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  outline: none;
  cursor: grab;
  user-select: none;
  touch-action: none;
  /* Checkerboard so transparent pixels stay visible. */
  background-color: #2a2a2a;
  background-image:
    linear-gradient(45deg, #333 25%, transparent 25%),
    linear-gradient(-45deg, #333 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, #333 75%),
    linear-gradient(-45deg, transparent 75%, #333 75%);
  background-size: 16px 16px;
  background-position: 0 0, 0 8px, 8px -8px, -8px 0;
}
.image-viewer.dragging { cursor: grabbing; }
img {
  position: absolute;
  left: 50%;
  top: 50%;
  max-width: none;
  max-height: none;
  transform-origin: center center;
  pointer-events: none;
}
.message {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #888;
}
.controls {
  position: absolute;
  left: 50%;
  bottom: 12px;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 6px;
  background: rgba(30, 30, 30, 0.88);
  border: 1px solid #444;
  border-radius: 6px;
  font-size: 11px;
  color: #ccc;
  cursor: default;
  opacity: 0.55;
  transition: opacity 0.15s;
}
.image-viewer:hover .controls,
.image-viewer:focus-within .controls { opacity: 1; }
.controls button {
  min-width: 24px;
  background: #1e1e1e;
  color: #ddd;
  border: 1px solid #555;
  border-radius: 3px;
  padding: 2px 6px;
  cursor: pointer;
}
.controls button:hover { border-color: #888; }
.controls button.active { border-color: #4ea1ff; color: #fff; }
.percent { min-width: 40px; text-align: center; font-variant-numeric: tabular-nums; }
.dims { color: #999; white-space: nowrap; }
.sep { width: 1px; height: 16px; background: #444; margin: 0 2px; }
</style>
