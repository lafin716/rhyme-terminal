import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { AccountProfile } from "./persistence";

/**
 * Fixed label for the floating OAuth login window opened from the Accounts
 * settings tab (see `openOAuthLoginWindow`). Kept to a single constant
 * (rather than one label per profile) so the Tauri capability that grants it
 * permissions (`src-tauri/capabilities/login-window.json`) can allowlist it
 * by exact window/webview label — one login flow at a time.
 */
export const OAUTH_LOGIN_WINDOW_LABEL = "oauth-login";

/**
 * Opens (or refocuses/replaces) the floating terminal window used to
 * complete an interactive OAuth login for `profile`. The window is a
 * separate JS realm — see `OAuthLoginWindow.vue`, mounted via the `win`
 * query param instead of `App.vue` (`src/main.ts`) — so profile identity is
 * passed through the URL rather than shared module state.
 */
export async function openOAuthLoginWindow(profile: AccountProfile): Promise<void> {
  const existing = await WebviewWindow.getByLabel(OAUTH_LOGIN_WINDOW_LABEL);
  if (existing) {
    await existing.close();
  }

  const params = new URLSearchParams({
    win: "oauth-login",
    agent: profile.agent,
    profileId: profile.id,
    label: profile.label,
    configDir: profile.configDir,
  });

  const win = new WebviewWindow(OAUTH_LOGIN_WINDOW_LABEL, {
    url: `/?${params.toString()}`,
    title: `${profile.label} — rhyme-terminal login`,
    width: 760,
    height: 480,
    resizable: true,
  });
  win.once("tauri://error", (e) => {
    console.error("Failed to open login window", e);
  });
}
