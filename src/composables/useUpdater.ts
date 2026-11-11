import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  isUpdateBusy,
  UPDATE_CHECK_INTERVAL_MS,
  UPDATE_FIRST_CHECK_MS,
  type UpdateInfo,
  type UpdateProgress,
  type UpdateStatus,
} from "../lib/updater";

// App-wide update state. Checks run automatically in the background and on
// demand from the status-bar version; installing is always the user's call
// because it restarts the app and closes running terminal sessions.
interface UpdaterState {
  status: UpdateStatus;
  info: UpdateInfo | null;
  progress: UpdateProgress | null;
  error: string | null;
  checkedAt: number | null;
}

const state = reactive<UpdaterState>({
  status: "idle",
  info: null,
  progress: null,
  error: null,
  checkedAt: null,
});

let autoTimers: number[] = [];
let autoUsers = 0;

/**
 * Asks the release endpoint for a newer version. A background check that
 * fails stays quiet (offline, no release yet) rather than flagging an error.
 */
async function check(options: { background?: boolean } = {}): Promise<void> {
  if (isUpdateBusy(state.status)) return;
  // A background poll must not hide an update the user has not acted on.
  if (options.background && state.status === "available") return;
  const previous = state.status;
  state.status = "checking";
  state.error = null;
  try {
    const info = await invoke<UpdateInfo | null>("update_check");
    state.info = info;
    state.status = info ? "available" : "up-to-date";
    state.checkedAt = Date.now();
  } catch (error) {
    if (options.background) {
      state.status = previous === "error" ? "idle" : previous;
    } else {
      state.status = "error";
      state.error = String(error);
    }
  }
}

/** Downloads, installs and restarts. Resolves only if something failed. */
async function install(): Promise<void> {
  if (state.status !== "available") return;
  state.status = "downloading";
  state.progress = { downloaded: 0, total: null };
  state.error = null;
  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await listen<UpdateProgress>("update-progress", (event) => {
      state.progress = event.payload;
      if (event.payload.total && event.payload.downloaded >= event.payload.total) {
        state.status = "installing";
      }
    });
    await invoke("update_install");
    state.status = "installing";
  } catch (error) {
    state.status = "error";
    state.error = String(error);
  } finally {
    unlisten?.();
  }
}

/** Starts background checks; returns a stop function for unmount. */
function startAutoCheck(): () => void {
  autoUsers++;
  if (autoUsers === 1) {
    autoTimers = [
      window.setTimeout(() => void check({ background: true }), UPDATE_FIRST_CHECK_MS),
      window.setInterval(() => void check({ background: true }), UPDATE_CHECK_INTERVAL_MS),
    ];
  }
  return () => {
    autoUsers--;
    if (autoUsers === 0) {
      for (const timer of autoTimers) { window.clearTimeout(timer); window.clearInterval(timer); }
      autoTimers = [];
    }
  };
}

export function useUpdater() {
  return { state, check, install, startAutoCheck };
}
