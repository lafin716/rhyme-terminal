import type { AccountProfile, CliAgentKind } from "../composables/useAccountProfiles";

/**
 * The profile to use by default for `agent`, or `null` for "System" (no env
 * override) — both when explicitly set that way and when the stored id no
 * longer matches any profile (e.g. it was removed since being chosen).
 */
export function resolveDefaultProfile(
  agent: CliAgentKind,
  profiles: AccountProfile[],
  defaultProfileId: Record<CliAgentKind, string | null>,
): AccountProfile | null {
  const id = defaultProfileId[agent];
  if (!id) return null;
  return profiles.find((p) => p.id === id && p.agent === agent) ?? null;
}
