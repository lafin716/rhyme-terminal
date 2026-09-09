import type { TerminalConfig } from "./terminal-config";

/**
 * Extends a new pane's startup args so the shell reports its cwd via an OSC 9;9
 * sequence on every prompt draw (used by `Terminal.vue`/`ExplorerPanel.vue` to
 * follow the active directory). No-op when the caller already passed args that
 * take over the shell's startup behavior (an explicit `-Command`/`-File`,
 * `/c`/`/k`, a non-default git-bash login invocation, or `-e`/`--exec` for
 * WSL) — those are returned unchanged (same array reference) so the caller
 * can tell the hook did not apply.
 */
export function withCwdIntegration(terminal: TerminalConfig, args: string[]): string[] {
  const lowerArgs = args.map((arg) => arg.toLowerCase());
  switch (terminal.preset) {
    case "windows-powershell":
    case "powershell":
      if (lowerArgs.some((arg) => arg === "-command" || arg === "-file")) return args;
      return [
        ...args,
        "-NoExit",
        "-Command",
        "$global:__winmuxPrompt=(Get-Command prompt).ScriptBlock; "
          + "function global:prompt { $p=(Get-Location).Path; "
          + "[Console]::Write(([char]27 + ']9;9;' + $p + [char]7)); "
          + "& $global:__winmuxPrompt }",
      ];
    case "cmd":
      if (lowerArgs.some((arg) => arg === "/c" || arg === "/k")) return args;
      return [...args, "/K", "prompt $E]9;9;$P$E\\$P$G"];
    case "git-bash":
      if (args.length > 0 && args.join("\0") !== "--login\0-i") return args;
      return [
        "-c",
        "export PROMPT_COMMAND='printf \"\\033]7;file:///%s\\033\\\\\" \"$(pwd -W)\"'; exec bash --login -i",
      ];
    case "wsl":
      if (lowerArgs.some((arg) => arg === "-e" || arg === "--exec")) return args;
      return [
        ...args,
        "sh",
        "-lc",
        "export PROMPT_COMMAND='printf \"\\033]7;file://wsl.localhost/%s%s\\033\\\\\" \"$WSL_DISTRO_NAME\" \"$PWD\"'; exec \"${SHELL:-bash}\" -l",
      ];
    case "custom":
    default:
      return args;
  }
}

/**
 * Splices an inline command (e.g. `"claude"`) into the startup args produced
 * by {@link withCwdIntegration}, so a freshly spawned pane launches straight
 * into that command instead of an empty prompt. Backs "New session" in the
 * Accounts settings tab (see `useAccountProfiles.ts`).
 *
 * `hookedArgs`/`argsForHook` must be the exact same call's input/output pair:
 * when `withCwdIntegration` didn't apply its hook (already-customized args —
 * an explicit `-Command`/`/K`/login flag, etc.) it returns the `args` it was
 * given unchanged, which this detects via reference equality and leaves
 * alone rather than guessing how to splice into an unknown script shape.
 */
export function withAgentLaunch(
  terminal: TerminalConfig,
  hookedArgs: string[],
  argsForHook: string[],
  launchCommand: string,
): string[] {
  if (hookedArgs === argsForHook || hookedArgs.length === 0) return hookedArgs;
  const command = launchCommand === "claude" ? "claude --permission-mode auto"
    : launchCommand === "codex" ? "codex --approve-for-me" : launchCommand;
  const out = [...hookedArgs];
  const last = out[out.length - 1];
  switch (terminal.preset) {
    case "windows-powershell":
    case "powershell":
      // last = the whole -Command script; script blocks chain with `;`.
      out[out.length - 1] = `${last}; ${command}`;
      return out;
    case "cmd":
      // last = the whole /K command string; `&` chains another command.
      out[out.length - 1] = `${last} & ${command}`;
      return out;
    case "git-bash":
    case "wsl":
      // last ends in "; exec ..." (replaces the shell to stay interactive);
      // run the launch command just before that so the shell survives it.
      out[out.length - 1] = last.includes("; exec ")
        ? last.replace("; exec ", `; ${command}; exec `)
        : `${last}; ${command}`;
      return out;
    case "custom":
    default:
      return hookedArgs;
  }
}
