<script setup lang="ts">
import { t } from "./composables/useI18n";
import { onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Terminal from "./components/Terminal.vue";
import { useSessions } from "./composables/useSessions";
import { envForProfile, launchCommandForProfile } from "./composables/useAccountProfiles";
import { loadAccountProfiles, type AccountProfile, type CliAgentKind } from "./lib/persistence";
import { api } from "./lib/tauri";

// This window is only ever opened by `openOAuthLoginWindow` (see
// `src/lib/floating-login.ts`) with these params, so the profile is
// reconstructed from the URL rather than an IPC round-trip — this window is
// a separate JS realm with no access to the main window's module state.
const params = new URLSearchParams(window.location.search);
const profile: AccountProfile = {
  id: params.get("profileId") ?? "",
  agent: (params.get("agent") as CliAgentKind | null) ?? "claude",
  label: params.get("label") ?? "",
  configDir: params.get("configDir") ?? "",
  createdAt: 0,
  authMethod: "oauth",
};

const { create } = useSessions();
const sessionId = ref<string | null>(null);
const error = ref<string | null>(null);

onMounted(async () => {


  const info = await create({
    env: envForProfile(loadAccountProfiles()?.find(p => p.id === profile.id && p.agent === profile.agent) ?? profile),
    launchCommand: launchCommandForProfile(profile),
    showError: false,
  });
  if (!info) {
    error.value = t("Could not start a terminal for this profile.");
    return;
  }
  sessionId.value = info.id;

  const win = getCurrentWindow();
  await win.onCloseRequested(async (event) => {
    event.preventDefault();
    if (sessionId.value) {
      await api.killSession(sessionId.value).catch(() => {});
    }
    await win.destroy();
  });
});
</script>

<template>
  <div class="login-window">
    <div class="hint">{{ t("Log in with") }} <b>{{ profile.label }}</b>{{ t(". Close this window once the login completes.") }}</div>
    <div class="term-host">
      <Terminal v-if="sessionId" :session-id="sessionId" :active="true" />
    </div>
    <div v-if="error" class="error">{{ error }}</div>
  </div>
</template>

<style>
html, body, #app {
  margin: 0;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: #1e1e1e;
  color: #d4d4d4;
  font-family: Inter, "Segoe UI", sans-serif;
}
* { box-sizing: border-box; }
</style>

<style scoped>
.login-window {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
}
.hint {
  flex-shrink: 0;
  padding: 8px 12px;
  font-size: 12px;
  color: #aaa;
  border-bottom: 1px solid #111;
  background: #202020;
}
.term-host {
  flex: 1;
  min-height: 0;
  padding: 4px;
}
.error {
  flex-shrink: 0;
  padding: 8px 12px;
  color: #e88;
  font-size: 12px;
}
</style>
