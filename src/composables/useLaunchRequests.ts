import { onUnmounted } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { api, onLaunchRequest, stringToBase64 } from "../lib/tauri";
import {
  isActionableLaunch,
  launchSessionOptions,
  type LaunchRequest,
} from "../lib/launch-request";
import { formatPathsForShell } from "../lib/path-insert";
import { presetForProgram } from "../lib/terminal-config";
import { useSessions } from "./useSessions";

/**
 * Acts on launch requests from outside the app.
 *
 * Every integration — Explorer's context menus, the command line, an external
 * tool, a `rhyme://` link — reaches this one handler as a `LaunchRequest`, so
 * adding an integration never touches session handling. What happens here is
 * only ever "open a tab the way the user would have": the same
 * `useSessions().create()` the New Session button calls.
 *
 * Two sources have to be drained, both of them the same requests:
 *  - `takeLaunchRequests`, for what arrived before this page could listen —
 *    the command line this process started with, and any click that landed
 *    during startup;
 *  - the `launch-request` event, for everything after.
 */
export function useLaunchRequests() {
  const sessions = useSessions();
  let unlisten: UnlistenFn | undefined;
  let queue: Promise<unknown> = Promise.resolve();

  /**
   * Requests are handled one at a time. Two context-menu clicks in quick
   * succession would otherwise race for the focused leaf and both land in the
   * same tab slot; serializing gives one tab each, in the order clicked.
   */
  function enqueue(request: LaunchRequest) {
    queue = queue.then(() => apply(request)).catch((error) => {
      console.error("launch request failed", error);
    });
  }

  async function apply(request: LaunchRequest) {
    if (!isActionableLaunch(request)) return;
    const { paths, ...options } = launchSessionOptions(request);
    const info = await sessions.create(options);
    if (!info || !paths.length) return;
    // The files a request named are typed at the prompt, never run — the same
    // contract a drop has. The shell decides the quoting; it is the one that
    // will parse this text.
    const text = formatPathsForShell(paths, presetForProgram(info.shell), info.shell);
    if (!text) return;
    await api.writeSession(info.id, stringToBase64(text));
  }

  /**
   * Begin handling requests, and settle the ones already waiting.
   *
   * Awaiting the first drain is what lets startup tell "opened with nothing to
   * do" from "opened because a folder was right-clicked": the caller can then
   * skip the empty default session it would otherwise add beside the one the
   * user actually asked for.
   */
  async function start() {
    unlisten = await onLaunchRequest(enqueue);
    // Only after the listener is live: a request that lands in between is
    // parked by the backend and comes back in this drain.
    for (const request of await api.takeLaunchRequests()) enqueue(request);
    await queue;
  }

  onUnmounted(() => unlisten?.());
  return { start, enqueue };
}
