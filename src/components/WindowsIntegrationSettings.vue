<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { Icon } from '@iconify/vue';
import { api, type ShellIntegrationStatus } from '../lib/tauri';

/*
 * Explorer's context menus and the `rhyme://` scheme are registry entries, and
 * the app owns them rather than an installer: it is the only thing that knows
 * where it currently lives, so an update that moves the executable repoints
 * them on its next start. Everything is written under HKCU, so nothing here
 * needs elevation and removing the app's profile removes it all.
 */
const status = ref<ShellIntegrationStatus | null>(null);
const busy = ref(false);
const error = ref('');

async function load() {
  try {
    status.value = await api.shellIntegrationStatus();
    error.value = '';
  } catch (e) {
    error.value = String(e);
  }
}
onMounted(load);

async function apply(next: { contextMenus?: boolean; uriScheme?: boolean }) {
  if (!status.value || busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    status.value = await api.shellIntegrationSet(
      next.contextMenus ?? status.value.contextMenus,
      next.uriScheme ?? status.value.uriScheme,
    );
  } catch (e) {
    error.value = String(e);
    await load();
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <div class="windows-integration">
    <header class="page-head">
      <h2>Windows 통합</h2>
      <p>
        탐색기 마우스 오른쪽 메뉴와 <code>rhyme://</code> 링크로 Rhyme Terminal을 열 수 있습니다.
        모두 현재 사용자 계정(HKCU)에만 등록되므로 관리자 권한이 필요 없습니다.
      </p>
    </header>

    <p v-if="error" class="banner" role="alert"><Icon icon="lucide:triangle-alert" />{{ error }}</p>

    <section class="card">
      <div class="card-head"><Icon icon="lucide:mouse-pointer-click" /><h3>탐색기 메뉴</h3></div>
      <label class="toggle">
        <input
          type="checkbox"
          :checked="status?.contextMenus ?? false"
          :disabled="busy || !status"
          @change="apply({ contextMenus: ($event.target as HTMLInputElement).checked })"
        />
        <span class="toggle-text">
          <span class="toggle-label">“Rhyme Terminal에서 열기” 메뉴 추가</span>
          <span class="toggle-hint">
            폴더 · 폴더 빈 공간 · 파일을 마우스 오른쪽 버튼으로 클릭하면 나타납니다.
            파일에서 열면 그 파일이 있는 폴더에서 터미널이 시작합니다.
          </span>
        </span>
      </label>
      <ul class="targets">
        <li><Icon icon="lucide:folder" />폴더 우클릭</li>
        <li><Icon icon="lucide:square-dashed" />폴더 빈 공간 우클릭</li>
        <li><Icon icon="lucide:file" />파일 우클릭</li>
      </ul>
    </section>

    <section class="card">
      <div class="card-head"><Icon icon="lucide:link" /><h3>rhyme:// 링크</h3></div>
      <label class="toggle">
        <input
          type="checkbox"
          :checked="status?.uriScheme ?? false"
          :disabled="busy || !status"
          @change="apply({ uriScheme: ($event.target as HTMLInputElement).checked })"
        />
        <span class="toggle-text">
          <span class="toggle-label">rhyme:// 주소를 이 앱으로 열기</span>
          <span class="toggle-hint">
            <code>rhyme://open?cwd=D:\workspace</code> ·
            <code>rhyme://ssh?host=example.com&amp;user=test</code>
          </span>
        </span>
      </label>
    </section>

    <section class="card">
      <div class="card-head"><Icon icon="lucide:terminal" /><h3>명령줄</h3></div>
      <p class="note">다른 프로그램에서 아래 형식으로 실행하면 같은 방식으로 열립니다. 이미 실행 중이면 새 탭으로 열립니다.</p>
      <pre><code>rhyme-terminal.exe --cwd "D:\workspace\project"
rhyme-terminal.exe --file "D:\workspace\project\README.md"
rhyme-terminal.exe --command "git status"
rhyme-terminal.exe --ssh "user@host"
rhyme-terminal.exe --new-window --cwd "D:\workspace"</code></pre>
      <p v-if="status?.executable" class="path">
        <Icon icon="lucide:file-code" />현재 실행 파일 <code>{{ status.executable }}</code>
      </p>
    </section>
  </div>
</template>
<style scoped>
.windows-integration {
  display: flex;
  flex-direction: column;
  gap: 24px;
  min-width: 0;
  color: #d4d4d4;
}
h2 {
  margin: 0 0 6px;
  font-size: 20px;
  font-weight: 600;
  color: #e6e6e6;
}
h3 {
  margin: 0;
  font-size: 14.5px;
  font-weight: 600;
  color: #e6e6e6;
}
.windows-integration p {
  margin: 0;
  max-width: 82ch;
  color: #aaaaaa;
  font-size: 13px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}
.card {
  min-width: 0;
  background: #202020;
  border: 1px solid #2b2b2b;
  border-radius: 10px;
  padding: 24px;
}
.card-head {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-bottom: 18px;
  color: #8a8a8a;
  font-size: 15px;
}
.banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-radius: 8px;
  color: #e98686 !important;
  background: rgba(233, 134, 134, 0.09);
  border: 1px solid rgba(233, 134, 134, 0.28);
  font-size: 13px;
}
.toggle {
  display: flex;
  align-items: flex-start;
  gap: 11px;
  cursor: pointer;
}
.toggle input {
  width: 16px;
  height: 16px;
  margin: 2px 0 0;
  flex-shrink: 0;
  accent-color: var(--accent);
}
.toggle-text {
  display: grid;
  gap: 5px;
  min-width: 0;
}
.toggle-label {
  font-size: 13px;
  color: #e6e6e6;
}
.toggle-hint {
  color: #8a8a8a;
  font-size: 12px;
  line-height: 1.65;
}
.targets {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  margin: 16px 0 0 27px;
  padding: 0;
  list-style: none;
}
.targets li {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: #8a8a8a;
  font-size: 12px;
}
.note {
  margin-bottom: 14px !important;
  font-size: 12.5px !important;
}
pre {
  margin: 0;
  padding: 14px;
  border-radius: 8px;
  background: #1a1a1a;
  border: 1px solid #2b2b2b;
  overflow-x: auto;
}
code {
  font-family: Consolas, "Cascadia Mono", monospace;
  font-size: 12px;
  color: #c8c8c8;
}
.path {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 14px !important;
  font-size: 12px !important;
  color: #808080 !important;
}
</style>
