import type { CliAgentKind, Prefs } from "./persistence";

export interface EnvRow { name: string; value: string }

export const ENV_PRESETS: Record<CliAgentKind, string[]> = {
  claude: ["ANTHROPIC_BASE_URL", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_MODEL", "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"],
  codex: ["OPENAI_BASE_URL", "OPENAI_API_KEY", "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"],
};

export function envError(rows: EnvRow[], reserved: string[] = []): string | null {
  const seen = new Set<string>();
  for (const row of rows) {
    const name = row.name.trim().toUpperCase();
    if (!/^[A-Z_][A-Z0-9_]*$/.test(name)) return "Use letters, numbers and underscores; start with a letter or underscore.";
    if (reserved.includes(name)) return "This variable is managed by the profile's login settings.";
    if (seen.has(name)) return "Variable names must be unique (case-insensitive).";
    if (row.value.includes("\0")) return "Values cannot contain a null character.";
    seen.add(name);
  }
  return null;
}

export function sessionAccountEnv(command: string | undefined, env: Record<string, string> | undefined, system: Prefs["systemAccountEnv"]): Record<string, string> | undefined {
  const agent = command === "claude" || command === "codex" ? command : null;
  if (!agent || env?.[agent === "claude" ? "CLAUDE_CONFIG_DIR" : "CODEX_HOME"]) return env;
  return { ...system?.[agent], ...env };
}
