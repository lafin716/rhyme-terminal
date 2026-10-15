export type TerminalPreset =
  | "windows-powershell"
  | "powershell"
  | "cmd"
  | "wsl"
  | "git-bash"
  | "zsh"
  | "custom";

export interface TerminalConfig {
  preset: TerminalPreset;
  program: string;
  args: string[];
}

export type TerminalPlatform = "windows" | "mac" | "any";

export interface TerminalPresetOption {
  id: TerminalPreset;
  label: string;
  program: string;
  args: string[];
  /** Host the preset's program exists on; `"any"` presets are offered everywhere. */
  platform: TerminalPlatform;
}

export const TERMINAL_PRESETS: TerminalPresetOption[] = [
  {
    id: "windows-powershell",
    label: "Windows PowerShell",
    program: "powershell.exe",
    args: [],
    platform: "windows",
  },
  {
    id: "powershell",
    label: "PowerShell 7",
    program: "pwsh.exe",
    args: [],
    platform: "windows",
  },
  {
    id: "cmd",
    label: "Command Prompt",
    program: "cmd.exe",
    args: [],
    platform: "windows",
  },
  {
    id: "wsl",
    label: "WSL",
    program: "wsl.exe",
    args: [],
    platform: "windows",
  },
  {
    id: "git-bash",
    label: "Git Bash",
    program: "C:\\Program Files\\Git\\bin\\bash.exe",
    args: ["--login", "-i"],
    platform: "windows",
  },
  { id: "zsh", label: "Zsh (macOS)", program: "/bin/zsh", args: ["-l", "-i"], platform: "mac" },
  {
    id: "custom",
    label: "Custom",
    program: "",
    args: [],
    platform: "any",
  },
];

/** Whether the UI runs on macOS (the only non-Windows host winmux ships for). */
export function isMacHost(): boolean {
  return typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
}

/**
 * Presets whose program can exist on this host: Windows shells are hidden on
 * macOS and the macOS shell is hidden on Windows. `selected` keeps an already
 * configured preset listed even when it belongs to the other host, so a synced
 * or imported setting never renders as an empty selection.
 */
export function availableTerminalPresets(
  selected?: TerminalPreset,
  mac = isMacHost(),
): TerminalPresetOption[] {
  const host: TerminalPlatform = mac ? "mac" : "windows";
  return TERMINAL_PRESETS.filter(
    (preset) => preset.platform === "any" || preset.platform === host || preset.id === selected,
  );
}

/**
 * The preset a running session's shell belongs to, recovered from the program
 * it was spawned with.
 *
 * A `SessionInfo` carries the executable, not the preset it came from, so a
 * consumer that needs to know how this shell parses text — path quoting, above
 * all — matches the program back to a preset here. Anything unrecognised is
 * `custom`, which the quoting layer then guesses from the program name.
 */
export function presetForProgram(program: string): TerminalPreset {
  const name = program.toLowerCase().replace(/\\/g, "/").split("/").pop() ?? "";
  const stem = name.replace(/\.exe$/, "");
  switch (stem) {
    case "powershell":
      return "windows-powershell";
    case "pwsh":
      return "powershell";
    case "cmd":
      return "cmd";
    case "wsl":
    case "wslhost":
      return "wsl";
    case "zsh":
      return "zsh";
    case "bash":
    case "sh":
      // Git Bash is the only bash a Windows install offers by default, and on
      // macOS the POSIX quoting it selects is the same either way.
      return "git-bash";
    default:
      return "custom";
  }
}

export function defaultTerminalConfig(): TerminalConfig {
  return configForPreset(isMacHost() ? "zsh" : "windows-powershell");
}

export function configForPreset(preset: TerminalPreset): TerminalConfig {
  const item = TERMINAL_PRESETS.find((candidate) => candidate.id === preset)
    ?? TERMINAL_PRESETS[0];
  return {
    preset: item.id,
    program: item.program,
    args: [...item.args],
  };
}

export function normalizeTerminalConfig(
  value: Partial<TerminalConfig> | null | undefined,
  fallback = defaultTerminalConfig(),
): TerminalConfig {
  const requestedPreset = value?.preset;
  const preset = requestedPreset
    && TERMINAL_PRESETS.some((item) => item.id === requestedPreset)
    ? requestedPreset
    : fallback.preset;
  return {
    preset,
    program: typeof value?.program === "string" ? value.program : fallback.program,
    args: Array.isArray(value?.args)
      ? value.args.filter((arg): arg is string => typeof arg === "string")
      : [...fallback.args],
  };
}

export function cloneTerminalConfig(config: TerminalConfig): TerminalConfig {
  return {
    preset: config.preset,
    program: config.program,
    args: [...config.args],
  };
}
