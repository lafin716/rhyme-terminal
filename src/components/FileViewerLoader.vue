<script setup lang="ts">
import { computed, defineAsyncComponent, onErrorCaptured, ref } from "vue";
import type { FilePreview } from "../lib/tauri";
import { useResources } from "../composables/useResources";
import { t } from "../composables/useI18n";
import { useKeybindings } from "../composables/useKeybindings";
import { matchesEvent } from "../lib/keybindings";

const props = defineProps<{ preview: FilePreview; tabId: string }>();
const resources = useResources();
const { bindingFor } = useKeybindings();
const failed = ref(false);
const loaded = ref(false);
const saveError = ref(false);
const tab = computed(() => {
  const resource = resources.getById(props.tabId);
  return resource?.kind === "file" ? resource : undefined;
});
const FileViewer = defineAsyncComponent({
  loader: () => import("./FileViewer.vue"),
  timeout: 15000,
});

// Catch both chunk-load failures and editor initialization errors locally.
// Drafts belong to the resource, so the fallback can safely continue editing.
onErrorCaptured(() => {
  failed.value = true;
  return false;
});

async function save() {
  try {
    await resources.saveFile(props.tabId);
    saveError.value = false;
  } catch {
    saveError.value = true;
  }
}

function onShortcut(event: KeyboardEvent) {
  if (event.isComposing || !matchesEvent(bindingFor("editor.save"), event)) return;
  event.preventDefault();
  event.stopPropagation();
  void save();
}
</script>

<template>
  <FileViewer v-if="!failed" :preview="preview" :tab-id="tabId" @vue:mounted="loaded = true" />
  <div v-if="failed || !loaded" class="viewer-status" @keydown.capture="onShortcut">
    <template v-if="failed && preview.kind === 'text'">
      <div class="fallback-toolbar">
        <span>{{ preview.canonicalPath || preview.name }}</span>
        <button :disabled="tab?.saving" @click="save">{{ t("Save") }}</button>
        <span v-if="saveError" role="alert">{{ t("Save failed") }}</span>
      </div>
      <div class="notice" role="status">{{ t("The editor could not be loaded. You can still edit and save here.") }}</div>
      <textarea
        class="fallback-editor"
        :aria-label="t('File content')"
        :value="tab?.draftText ?? preview.text ?? ''"
        spellcheck="false"
        @input="resources.updateFileDraft(tabId, ($event.target as HTMLTextAreaElement).value)"
      />
    </template>
    <div v-else class="notice" role="status">{{ t(failed ? "Failed to load file preview." : "Loading…") }}</div>
  </div>
</template>

<style scoped>
.viewer-status { position: absolute; inset: 0; display: flex; flex-direction: column; background: #1e1e1e; color: #d4d4d4; }
.fallback-toolbar { display: flex; align-items: center; gap: 12px; min-height: 32px; padding: 0 8px; background: #252525; font-size: 12px; }
.fallback-toolbar > span:first-child { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
button { background: #1e1e1e; color: #ddd; border: 1px solid #555; cursor: pointer; }
.notice { padding: 12px; font-size: 12px; }
.fallback-editor { flex: 1; min-height: 0; resize: none; padding: 12px; border: 0; outline: none; background: #1e1e1e; color: #d4d4d4; font: 13px/19px "Cascadia Mono", Consolas, monospace; tab-size: 4; }
</style>
