<script setup lang="ts">
import { t } from "../composables/useI18n";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watchEffect } from "vue";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api.js";
import "monaco-editor/esm/vs/language/json/monaco.contribution";
import "monaco-editor/esm/vs/basic-languages/bat/bat.contribution";
import "monaco-editor/esm/vs/basic-languages/css/css.contribution";
import "monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution";
import "monaco-editor/esm/vs/basic-languages/html/html.contribution";
import "monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution";
import "monaco-editor/esm/vs/basic-languages/powershell/powershell.contribution";
import "monaco-editor/esm/vs/basic-languages/python/python.contribution";
import "monaco-editor/esm/vs/basic-languages/rust/rust.contribution";
import "monaco-editor/esm/vs/basic-languages/shell/shell.contribution";
import "monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution";
import "monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import JsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import MarkdownIt from "markdown-it";
import { api, base64ToBytes, type FilePreview } from "../lib/tauri";
import { resolveViewerMode } from "../lib/viewer-mode";
import { useResources } from "../composables/useResources";

const props = defineProps<{ preview: FilePreview; tabId: string }>();

const resources = useResources();

const editorHost = ref<HTMLDivElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let model: monaco.editor.ITextModel | null = null;

// Baseline the dirty check compares against: the last text written to disk
// (initially the loaded content). Ctrl+S advances it on a successful save.
let lastSaved = props.preview.text ?? "";
const saveError = ref<string | null>(null);

// `html: false` keeps embedded raw HTML/scripts inert — markdown-it escapes
// them rather than emitting them, so opening an untrusted file is safe.
const md = new MarkdownIt({ html: false, linkify: true, breaks: false });

const mode = computed(() => resolveViewerMode(props.preview));

const imageSrc = computed(() =>
  props.preview.data ? `data:${props.preview.mime};base64,${props.preview.data}` : "",
);

const markdownHtml = computed(() =>
  mode.value === "markdown" ? md.render(props.preview.text ?? "") : "",
);

// Feed the PDF to the webview's native viewer via a same-origin blob URL built
// from the base64 bytes the backend shipped. The object URL is revoked when the
// preview changes or the component unmounts (via watchEffect's cleanup).
const pdfSrc = ref("");
watchEffect((onCleanup) => {
  if (mode.value !== "pdf" || !props.preview.data) {
    pdfSrc.value = "";
    return;
  }
  const blob = new Blob([base64ToBytes(props.preview.data)], { type: "application/pdf" });
  const url = URL.createObjectURL(blob);
  pdfSrc.value = url;
  onCleanup(() => URL.revokeObjectURL(url));
});

const workerScope = self as typeof self & {
  MonacoEnvironment?: {
    getWorker(moduleId: string, label: string): Worker;
  };
};
workerScope.MonacoEnvironment = {
  getWorker(_moduleId, label) {
    if (label === "json") return new JsonWorker();
    return new EditorWorker();
  },
};

function monacoLanguage(language: string): string {
  if (language === "vue") return "html";
  if (language === "shell") return "shell";
  if (language === "batch") return "bat";
  return language;
}

async function createEditor() {
  if (mode.value !== "text") return;
  await nextTick();
  if (!editorHost.value) return;
  model = monaco.editor.createModel(
    props.preview.text ?? "",
    monacoLanguage(props.preview.language),
    monaco.Uri.file(props.preview.canonicalPath),
  );
  // Only the `text` view mode reaches here (early return above), so the editor
  // is always editable; Markdown/PDF/image are separate read-only views.
  editor = monaco.editor.create(editorHost.value, {
    model,
    readOnly: false,
    domReadOnly: false,
    theme: "vs-dark",
    automaticLayout: true,
    minimap: { enabled: false },
    fontFamily: '"Cascadia Mono", Consolas, monospace',
    fontSize: 13,
    lineHeight: 19,
    renderWhitespace: "selection",
    scrollBeyondLastLine: false,
    smoothScrolling: true,
  });
  if (props.preview.line) {
    editor.setPosition({
      lineNumber: props.preview.line,
      column: props.preview.column ?? 1,
    });
    editor.revealLineInCenter(props.preview.line);
  }

  // Dirty = current text differs from the last-saved baseline.
  editor.onDidChangeModelContent(() => {
    resources.setFileDirty(props.tabId, model?.getValue() !== lastSaved);
  });

  // Ctrl+S saves; binding it on the editor also suppresses the browser's own
  // save dialog while the editor is focused.
  editor.addAction({
    id: "winmux.saveFile",
    label: t("Save File"),
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS],
    run: () => {
      void save();
    },
  });
}

async function save() {
  if (!model) return;
  const text = model.getValue();
  try {
    await api.writeFile(props.preview.canonicalPath, text);
    lastSaved = text;
    saveError.value = null;
    // Adopts the saved text as the tab's stored content and clears dirty.
    resources.updateFilePreviewText(props.tabId, text);
  } catch (e) {
    // Surface the failure and keep the dirty flag so the edit isn't lost.
    saveError.value = String(e);
    console.error("Failed to save file:", e);
  }
}

function openFind() {
  editor?.getAction("actions.find")?.run();
}

onMounted(createEditor);
onBeforeUnmount(() => {
  editor?.dispose();
  editor = null;
  model?.dispose();
  model = null;
});
</script>

<template>
  <div class="file-viewer">
    <div class="toolbar">
      <span class="path" :title="preview.canonicalPath">{{ preview.canonicalPath }}</span>
      <button v-if="mode === 'text'" :title="t('Find (Ctrl+F)')" @click="openFind">{{ t("Find") }}</button>
      <span v-if="saveError" class="save-error" :title="saveError">{{ t("Save failed") }}</span>
      <span class="meta">{{ preview.language }} · {{ preview.size.toLocaleString() }} {{ t("bytes") }}</span>
    </div>

    <div v-if="mode === 'image'" class="image-wrap">
      <img :src="imageSrc" :alt="preview.name" />
    </div>
    <!-- Rendered read-only Markdown; markdown-it emits safe HTML (html: false). -->
    <div v-else-if="mode === 'markdown'" class="markdown-body" v-html="markdownHtml" />
    <iframe
      v-else-if="mode === 'pdf'"
      class="pdf-frame"
      :src="pdfSrc"
      :title="preview.name"
    />
    <div v-else-if="mode === 'binary'" class="message">{{ t("Binary files cannot be previewed.") }}</div>
    <div v-else-if="mode === 'too_large'" class="message">{{ t("This file is too large to preview.") }}</div>
    <div v-else ref="editorHost" class="editor-host" />
  </div>
</template>

<style scoped>
.file-viewer {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: #1e1e1e;
  color: #d4d4d4;
}
.toolbar {
  height: 32px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 8px;
  background: #252525;
  border-bottom: 1px solid #111;
  font-size: 11px;
}
.path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}
.meta { color: #888; white-space: nowrap; }
.save-error {
  color: #f48771;
  white-space: nowrap;
  cursor: default;
}
button {
  background: #1e1e1e;
  color: #ddd;
  border: 1px solid #555;
  padding: 3px 6px;
  cursor: pointer;
}
.editor-host {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.image-wrap, .message {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: auto;
  color: #888;
}
img { max-width: 100%; max-height: 100%; object-fit: contain; }

.pdf-frame {
  flex: 1;
  min-height: 0;
  width: 100%;
  border: 0;
  background: #333;
}

.markdown-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 16px 24px;
  line-height: 1.6;
  font-family: -apple-system, "Segoe UI", system-ui, sans-serif;
  font-size: 14px;
}
/* v-html content is not scoped, so reach it with :deep(). */
.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4) {
  color: #fff;
  border-bottom: 1px solid #333;
  padding-bottom: 4px;
  margin: 20px 0 12px;
}
.markdown-body :deep(a) { color: #4ea1ff; }
.markdown-body :deep(code) {
  background: #2a2a2a;
  padding: 1px 5px;
  border-radius: 3px;
  font-family: "Cascadia Mono", Consolas, monospace;
  font-size: 12px;
}
.markdown-body :deep(pre) {
  background: #252525;
  padding: 12px;
  border-radius: 4px;
  overflow: auto;
}
.markdown-body :deep(pre code) { background: none; padding: 0; }
.markdown-body :deep(blockquote) {
  border-left: 3px solid #555;
  margin: 12px 0;
  padding: 0 12px;
  color: #aaa;
}
.markdown-body :deep(table) { border-collapse: collapse; }
.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid #444;
  padding: 4px 10px;
}
.markdown-body :deep(img) { max-width: 100%; }
.markdown-body :deep(hr) { border: none; border-top: 1px solid #333; }
</style>
