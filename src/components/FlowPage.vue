<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import FlowPanel from "./FlowPanel.vue";
import { useFlowPage } from "../composables/useFlowPage";
import { t } from "../composables/useI18n";

defineProps<{ project: string; projectId: string; projectName: string }>();
const { flowOpen, closeFlow } = useFlowPage();
const closeButton = ref<HTMLButtonElement>();
const sections = [
  { id: 'designer', label: 'Flow 디자이너' },
  { id: 'workers', label: '워커' },
  { id: 'tasks', label: '프로젝트 채팅' },
  { id: 'runs', label: '실행 기록' },
] as const;
const activeTab = ref<typeof sections[number]['id']>('designer');
let previousFocus: HTMLElement | null = null;

watch(flowOpen, async (open) => {
  if (open) {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await nextTick();
    closeButton.value?.focus();
  } else {
    await nextTick();
    previousFocus?.focus();
  }
}, { immediate: true });

function onKeyDown(event: KeyboardEvent) {
  // Keep keyboard input inside this page while the terminal stays mounted underneath.
  event.stopPropagation();
  if (event.key === "Escape" && !event.defaultPrevented) {
    event.preventDefault();
    closeFlow();
  }
}
</script>

<template>
  <section v-show="flowOpen" class="flow-page" aria-labelledby="flow-page-title" @keydown="onKeyDown">
    <aside class="page-sidebar">
      <button ref="closeButton" type="button" class="back-to-app" @click="closeFlow">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m12 5-7 7 7 7M5 12h14" /></svg>
        <span>{{ t('Back to app') }}</span>
      </button>
      <div class="heading">
        <h1 id="flow-page-title">Rhyme Flow</h1>
        <span class="project-name" :title="project">{{ projectName }}</span>
      </div>
      <nav aria-label="Rhyme Flow">
        <button v-for="item in sections" :key="item.id" type="button" class="nav-item" :class="{ active: activeTab === item.id }" :aria-current="activeTab === item.id ? 'page' : undefined" @click="activeTab = item.id">{{ item.label }}</button>
      </nav>
    </aside>
    <FlowPanel :key="projectId" v-model:tab="activeTab" :project="project" :project-id="projectId" :active="flowOpen" />
  </section>
</template>

<style scoped>
.flow-page { position: fixed; inset: var(--titlebar-height, 36px) 0 0; z-index: 1000; display: flex; background: #1e1e1e; color: #d4d4d4; }
.page-sidebar { width: 280px; flex: 0 0 280px; min-height: 0; overflow-y: auto; background: #1b1b1b; border-right: 1px solid #111; }
.back-to-app { display: flex; align-items: center; gap: 10px; margin: 12px; padding: 10px 12px; border: 0; border-radius: 6px; background: transparent; color: #d4d4d4; font: inherit; font-size: 13px; text-align: left; cursor: pointer; }
.back-to-app:hover { background: #282828; color: #fff; }
.back-to-app svg { flex-shrink: 0; }
.heading { display: flex; flex-direction: column; gap: 8px; margin: 6px 26px 14px; min-width: 0; }
h1 { font-size: 13px; font-weight: 600; margin: 0; color: #aaa; }
.project-name { color: #999; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
nav { display: flex; flex-direction: column; gap: 2px; padding: 0 12px 20px; }
.nav-item { padding: 11px 14px; border: 0; border-left: 3px solid transparent; border-radius: 8px; background: transparent; color: #aaa; text-align: left; font: inherit; font-size: 13px; cursor: pointer; }
.nav-item:hover { background: #202020; }
.nav-item.active { background: #202020; border-left-color: #4ec9b0; color: #4ec9b0; }
button:focus-visible { outline: 2px solid #4ec9b0; outline-offset: 2px; }
.flow-page :deep(.flow-panel) { flex: 1; min-width: 0; height: auto; }
@media (max-width: 760px) {
  .flow-page { flex-direction: column; }
  .page-sidebar { width: 100%; flex: 0 0 auto; border-right: 0; }
  .back-to-app { margin: 8px 12px 0; }
  .heading { flex-direction: row; align-items: baseline; margin: 8px 24px; }
  nav { flex-direction: row; padding: 8px; overflow-x: auto; border-bottom: 1px solid #333; }
  .nav-item { flex-shrink: 0; }
}
</style>
