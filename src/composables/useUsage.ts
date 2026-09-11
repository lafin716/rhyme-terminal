import { computed, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { CLI_AGENTS, useAccountProfiles } from "./useAccountProfiles";
import { usePrefs } from "./usePrefs";
import type { CliAgentKind } from "../lib/persistence";
import type { UsageStat } from "../lib/usage-status";

export interface UsageWindow extends UsageStat { label: string }
export interface ProfileUsage {
  windows: UsageWindow[];
  loading: boolean;
  error: string | null;
  updatedAt: number | null;
  attemptedAt: number;
}
export const usageKey = (agent: CliAgentKind, id: string | null) => `${agent}:${id ?? "system"}`;
const records = reactive<Record<string, ProfileUsage>>({});
const { profiles } = useAccountProfiles();
const { prefs, setPref } = usePrefs();
const targets = computed(() => CLI_AGENTS.flatMap(agent => [
  { agent: agent.id, id: null as string | null, label: "System", sessionUsage: false },
  ...profiles.filter(p => p.agent === agent.id).map(p => ({ agent: p.agent, id: p.id as string | null, label: p.label, sessionUsage: p.authMethod === "setup-token" })),
]));
const empty: UsageStat = { percentUsed: null, resetsAt: null };

export interface RefreshOptions {
  /** Only poll profiles whose usage comes from a local session sample. */
  sessionOnly?: boolean;
  /**
   * Restrict the sweep to these usage keys. Timed refreshes pass the profiles
   * with a live agent process (see `lib/usage-active.ts`); omit it to cover
   * every profile, which only user-initiated refreshes should do.
   */
  keys?: ReadonlySet<string>;
  /**
   * Skip the client-side throttle for an explicit, user-initiated check. The
   * daemon still answers from its own one-minute cache, so this cannot reach
   * the provider more often than a timed refresh would.
   */
  force?: boolean;
}

async function refresh(options: boolean | RefreshOptions = {}) {
  const { sessionOnly = false, keys, force = false } =
    typeof options === "boolean" ? { sessionOnly: options } as RefreshOptions : options;
  // Two workers bound concurrent network requests for large profile collections.
  const queue = targets.value.filter(target => (!sessionOnly || target.sessionUsage)
    && (!keys || keys.has(usageKey(target.agent, target.id))));
  const worker = async () => {
    for (let target = queue.shift(); target; target = queue.shift()) {
      const key = usageKey(target.agent, target.id);
      records[key] ??= { windows: [], loading: false, error: null, updatedAt: null, attemptedAt: 0 };
      const record = records[key];
      if (record.loading) continue;
      if (!force && Date.now() - record.attemptedAt < (target.sessionUsage ? 5_000 : 60_000)) continue;
      record.attemptedAt = Date.now();

      record.loading = true;
      try {
        const windows = await invoke<Array<{ label: string; percentUsed: number; resetsAt: string | number | null; receivedAt?: number }>>("get_account_usage", { agent: target.agent, profileId: target.id, sessionUsage: target.sessionUsage });
        record.windows = windows.map(window => {
          const reset = typeof window.resetsAt === "string" ? Date.parse(window.resetsAt) : window.resetsAt;
          return { ...window, resetsAt: reset !== null && Number.isFinite(reset) ? reset : null };
        });
        record.updatedAt = windows[0]?.receivedAt ?? Date.now();
        record.error = null;
      } catch (error) {
        if (target.sessionUsage) {
          record.windows = typeof error === "string" && error.startsWith("Waiting for session usage")
            ? [] : record.windows.filter(window => window.resetsAt !== null && window.resetsAt > Date.now());
          if (!record.windows.length) record.updatedAt = null;
        }
        record.error = typeof error === "string" ? error : "Usage service is temporarily unavailable";
      } finally {
        record.loading = false;
      }
    }
  };
  await Promise.all([worker(), worker()]);
}

export function useUsage() {
  const selected = (agent: CliAgentKind) => targets.value.find(p => p.agent === agent && p.id === (prefs.toolbarProfileId?.[agent] !== undefined ? prefs.toolbarProfileId[agent] : prefs.defaultProfileId[agent]))
    ?? targets.value.find(p => p.agent === agent && p.id === null)!;
  const selectToolbarProfile = (agent: CliAgentKind, id: string | null) => {
    if (!targets.value.some(profile => profile.agent === agent && profile.id === id)) return;
    setPref("toolbarProfileId", { ...prefs.toolbarProfileId, [agent]: id });
  };
  const summary = (agent: CliAgentKind) => records[usageKey(agent, selected(agent).id)];
  const usage = computed(() => ({ claude: summary("claude")?.windows[0] ?? empty, codex: summary("codex")?.windows[0] ?? empty }));
  const loading = computed(() => Object.values(records).some(record => record.loading));
  return { usage, records, targets, selected, selectToolbarProfile, summary, refresh, loading };
}
