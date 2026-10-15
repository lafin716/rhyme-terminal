<script setup lang="ts">
import { ref } from "vue";
import type { LayoutNode, SplitNode } from "../lib/layout-types";
import PaneTabs from "./PaneTabs.vue";

const props = defineProps<{ node: LayoutNode }>();

const rootRef = ref<HTMLDivElement | null>(null);

function isSplit(n: LayoutNode): n is SplitNode {
  return n.kind === "split";
}

function flexBasis(size: number): Record<string, string> {
  return { flexBasis: `${size * 100}%`, flexGrow: "0", flexShrink: "1" };
}

function onSplitterMouseDown(ev: MouseEvent, index: number) {
  if (!isSplit(props.node) || !rootRef.value) return;
  ev.preventDefault();
  const split = props.node;
  const containerRect = rootRef.value.getBoundingClientRect();
  const horizontal = split.direction === "horizontal";
  const total = horizontal ? containerRect.width : containerRect.height;
  const start = horizontal ? ev.clientX : ev.clientY;
  const originalSizes = [...split.sizes];

  function onMove(e: MouseEvent) {
    const current = horizontal ? e.clientX : e.clientY;
    const deltaPx = current - start;
    const deltaRatio = deltaPx / total;
    const a = originalSizes[index];
    const b = originalSizes[index + 1];
    const minRatio = 0.05;
    let newA = a + deltaRatio;
    let newB = b - deltaRatio;
    if (newA < minRatio) {
      newA = minRatio;
      newB = a + b - minRatio;
    }
    if (newB < minRatio) {
      newB = minRatio;
      newA = a + b - minRatio;
    }
    split.sizes[index] = newA;
    split.sizes[index + 1] = newB;
  }

  function onUp() {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }

  document.body.style.cursor = horizontal ? "col-resize" : "row-resize";
  document.body.style.userSelect = "none";
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}
</script>

<template>
  <div v-if="isSplit(node)" ref="rootRef" class="split" :class="node.direction">
    <template v-for="(child, i) in node.children" :key="child.id">
      <div class="split-child" :style="flexBasis(node.sizes[i])">
        <SplitContainer :node="child" />
      </div>
      <div
        v-if="i < node.children.length - 1"
        class="splitter"
        :class="node.direction"
        @mousedown="onSplitterMouseDown($event, i)"
      />
    </template>
  </div>
  <PaneTabs v-else :leaf="node" />
</template>

<style scoped>
.split {
  display: flex;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
}
.split.horizontal { flex-direction: row; }
.split.vertical { flex-direction: column; }
.split-child {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.splitter {
  background: #111;
  flex: 0 0 4px;
  z-index: 1;
}
.splitter.horizontal {
  cursor: col-resize;
  width: 4px;
}
.splitter.vertical {
  cursor: row-resize;
  height: 4px;
}
.splitter:hover {
  background: var(--accent);
}
</style>
