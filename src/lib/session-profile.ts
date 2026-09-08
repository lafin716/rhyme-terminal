import type { AccountProfile, CliAgentKind } from "./persistence";

export interface SessionProfile {
  agent: CliAgentKind;
  id: string | null;
  label: string;
}

export function profileForLaunch(
  profiles: AccountProfile[],
  env?: Record<string, string>,
  command?: string,
): SessionProfile | undefined {
  const agent = command === "claude" || command === "codex" ? command : undefined;
  const profile = profiles.find(p => (!agent || p.agent === agent)
    && env?.[p.agent === "claude" ? "CLAUDE_CONFIG_DIR" : "CODEX_HOME"] === p.configDir);
  if (profile) return { agent: profile.agent, id: profile.id, label: profile.label };
  if (agent && !env?.[agent === "claude" ? "CLAUDE_CONFIG_DIR" : "CODEX_HOME"])
    return { agent, id: null, label: "" };
  return undefined;
}
