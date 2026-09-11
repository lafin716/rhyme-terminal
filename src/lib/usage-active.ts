import type { AgentKind } from "./tauri";
import type { CliAgentKind } from "./persistence";
import type { SessionProfile } from "./session-profile";

export interface UsageProfileRef {
  agent: CliAgentKind;
  id: string | null;
}

/** One live PTY and the runtime agent kind the daemon last reported for it. */
export interface UsageSession {
  id: string;
  agent: AgentKind;
}

/**
 * One Agent Loop group. Its PTY is spawned by the daemon, so no frontend
 * launch snapshot exists for it — `activeProfile` (`"<agent>:<id|system>"`)
 * is the only profile attribution available.
 */
export interface UsageLoop {
  activeProfile?: string | null;
  /** Whether an Agent process is alive in the group right now. */
  live: boolean;
  /** Every PTY the group owns, current and historical. */
  sessionIds: string[];
}

const isCliAgent = (agent: string): agent is CliAgentKind =>
  agent === "claude" || agent === "codex";

/** Splits an `"<agent>:<id|system>"` usage key back into a profile reference. */
export function parseUsageKey(key: string): UsageProfileRef | undefined {
  const separator = key.indexOf(":");
  if (separator < 0) return undefined;
  const agent = key.slice(0, separator);
  const id = key.slice(separator + 1);
  if (!isCliAgent(agent) || !id) return undefined;
  return { agent, id: id === "system" ? null : id };
}

/**
 * Profiles spending quota right now — the only ones worth polling on a timer.
 * A PTY parked at a shell prompt consumes nothing, so refreshing its profile
 * only burns the provider's rate limit; those profiles are refreshed on
 * demand from the usage dialog instead (see `StatusBar.vue`).
 *
 * A session running a CLI agent with no recorded launch profile is attributed
 * to that agent's system login, which is what a bare `claude`/`codex` typed
 * into a plain terminal actually uses. Loop-owned PTYs are excluded from that
 * fallback because their profile comes from the group instead.
 */
export function activeUsageProfiles(
  sessions: UsageSession[],
  profileOf: (sessionId: string) => SessionProfile | undefined,
  loops: UsageLoop[] = [],
): UsageProfileRef[] {
  const found = new Map<string, UsageProfileRef>();
  const add = (ref: UsageProfileRef) => found.set(`${ref.agent}:${ref.id ?? "system"}`, ref);
  const managed = new Set(loops.flatMap(loop => loop.sessionIds));
  for (const loop of loops) {
    if (!loop.live || !loop.activeProfile) continue;
    const ref = parseUsageKey(loop.activeProfile);
    if (ref) add(ref);
  }
  for (const session of sessions) {
    if (!isCliAgent(session.agent) || managed.has(session.id)) continue;
    const profile = profileOf(session.id);
    add(profile && profile.agent === session.agent
      ? { agent: profile.agent, id: profile.id }
      : { agent: session.agent, id: null });
  }
  return [...found.values()];
}
