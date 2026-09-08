import { createApp, watchEffect } from "vue";
import { useI18n } from "./composables/useI18n";
import { loadPrefsFromStorage } from "./composables/usePrefs";
import { isTauri } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import App from "./App.vue";
import OAuthLoginWindow from "./OAuthLoginWindow.vue";

// The floating OAuth login window (see `src/lib/floating-login.ts`) is a
// separate Tauri window loading this same bundle; it's told apart from the
// main window purely via a `win` query param on the URL it's opened with,
// so it mounts a different, minimal root instead of the full app shell.
const params = new URLSearchParams(window.location.search);
const root = params.get("win") === "oauth-login" ? OAuthLoginWindow : App;

loadPrefsFromStorage();
const { locale } = useI18n();
watchEffect(() => {
  document.documentElement.lang = locale.value;
  if (root === App && isTauri()) {
    void emit("app-language-changed", locale.value)
      .catch((error) => console.warn("Failed to update tray language", error));
  }
});

createApp(root).mount("#app");
