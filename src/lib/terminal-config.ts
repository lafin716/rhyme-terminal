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

export interface TerminalPresetOption {
  id: TerminalPreset;
  label: string;
  program: string;
  args: string[];
}

export const TERMINAL_PRESETS: TerminalPresetOption[] = [
  {
    id: "windows-powershell",
    label: "Windows PowerShell",
    program: "powershell.exe",
    args: [],
  },
  {
    id: "powershell",
    label: "PowerShell 7",
    program: "pwsh.exe",
    args: [],
  },
  {
    id: "cmd",
    label: "Command Prompt",
    program: "cmd.exe",
    args: [],
  },
  {
    id: "wsl",
    label: "WSL",
    program: "wsl.exe",
    args: [],
  },
  {
    id: "git-bash",
    label: "Git Bash",
    program: "C:\\Program Files\\Git\\bin\\bash.exe",
    args: ["--login", "-i"],
  },
  { id: "zsh", label: "Zsh (macOS)", program: "/bin/zsh", args: ["-l", "-i"] },
  {
    id: "custom",
    label: "Custom",
    program: "",
    args: [],
  },
];

export function defaultTerminalConfig(): TerminalConfig {
  return configForPreset(typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "zsh" : "windows-powershell");
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
