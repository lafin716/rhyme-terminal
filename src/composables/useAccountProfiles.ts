import { reactive } from "vue";
import { api } from "../lib/tauri";
import {
  loadAccountProfiles,
  saveAccountProfiles,
  type AccountAuthMethod,
  type AccountProfile,
  type CliAgentKind,
} from "../lib/persistence";

export type { AccountAuthMethod, AccountProfile, CliAgentKind };

/** Static metadata for each supported CLI agent: its settings-tab label, the
 * env var it reads for its config/home directory, and the command that
 * launches it. Adding a third agent later is a one-line change here. */
export interface CliAgentDef {
  id: CliAgentKind;
  label: string;
  envVar: string;
  command: string;
}

export const CLI_AGENTS: CliAgentDef[] = [
  { id: "claude", label: "Claude Code", envVar: "CLAUDE_CONFIG_DIR", command: "claude" },
  { id: "codex", label: "Codex", envVar: "CODEX_HOME", command: "codex" },
];

function agentDef(agent: CliAgentKind): CliAgentDef {
  return CLI_AGENTS.find((a) => a.id === agent) ?? CLI_AGENTS[0];
}

const profiles = reactive<AccountProfile[]>([]);

function persist(): void {
  saveAccountProfiles(profiles.map((p) => ({ ...p })));
}

function genId(): string {
  return `acct_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 7)}`;
}

export function loadAccountProfilesFromStorage(): void {
  const stored = loadAccountProfiles();
  if (!stored) return;
  profiles.splice(0, profiles.length, ...stored);
}

export function profilesForAgent(agent: CliAgentKind): AccountProfile[] {
  return profiles.filter((p) => p.agent === agent);
}

/**
 * Creates a new account profile: resolves (and creates on disk) its isolated
 * login directory via the backend, then persists it. Throws if the directory
 * cannot be created (e.g. disk/permission error), leaving the list unchanged.
 */
export async function addAccountProfile(
  agent: CliAgentKind,
  label: string,
  authMethod: AccountAuthMethod = "oauth",
): Promise<AccountProfile> {
  const id = genId();
  const configDir = await api.resolveAccountDir(agent, id);
  const created: AccountProfile = {
    id,
    agent,
    label,
    configDir,
    createdAt: Date.now(),
    authMethod,
  };
  profiles.push(created);
  persist();
  return created;
}

export function renameAccountProfile(id: string, label: string): void {
  const p = profiles.find((x) => x.id === id);
  if (!p) return;
  p.label = label;
  persist();
}

/**
 * Removes a profile from the saved list. Its login directory on disk is left
 * untouched (re-adding a profile that reuses that folder would pick the login
 * back up), so this is safe to call without a "this deletes your login"
 * warning; callers still confirm since it's not obviously reversible from the
 * UI.
 */
export function removeAccountProfile(id: string): void {
  const idx = profiles.findIndex((x) => x.id === id);
  if (idx < 0) return;
  profiles.splice(idx, 1);
  persist();
}

/** Custom variables plus the isolated login directory, which takes precedence. */
export function envForProfile(profile: AccountProfile): Record<string, string> {
  return { ...profile.env, [agentDef(profile.agent).envVar]: profile.configDir };
}

export function setAccountProfileEnv(id: string, env: Record<string, string>): void {
  const profile = profiles.find((p) => p.id === id);
  if (!profile) return;
  profile.env = { ...env };
  persist();
}

/**
 * Full env override(s) for launching a session as this profile, including a
 * `"setup-token"`-linked token when the profile uses one. Async (reads the
 * token from disk via the backend) — prefer the sync {@link envForProfile}
 * when the profile is known to be `"oauth"` (e.g. inside the floating login
 * window, which only ever launches OAuth profiles).
 */
export async function resolveProfileEnv(profile: AccountProfile): Promise<Record<string, string>> {
  const env = envForProfile(profile);
  if (profile.agent === "claude" && profile.authMethod === "setup-token") {
    const token = await api.getAccountToken(profile.agent, profile.id);
    if (token) env.CLAUDE_CODE_OAUTH_TOKEN = token;
  }
  return env;
}

/** The shell command that launches this profile's agent (e.g. `"claude"`). */
export function launchCommandForProfile(profile: AccountProfile): string {
  return agentDef(profile.agent).command;
}

export function useAccountProfiles() {
  return { profiles };
}
