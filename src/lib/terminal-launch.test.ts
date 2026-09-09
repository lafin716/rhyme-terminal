import { describe, expect, it } from "vitest";
import { withAgentLaunch, withCwdIntegration } from "./terminal-launch";
import { configForPreset } from "./terminal-config";

describe("withCwdIntegration", () => {
  it("appends a -NoExit -Command prompt hook for PowerShell presets", () => {
    const terminal = configForPreset("powershell");
    const args = withCwdIntegration(terminal, []);
    expect(args[0]).toBe("-NoExit");
    expect(args[1]).toBe("-Command");
    expect(args[2]).toContain("function global:prompt");
  });

  it("leaves PowerShell args untouched when -Command is already present", () => {
    const terminal = configForPreset("powershell");
    const args = ["-Command", "Get-Process"];
    expect(withCwdIntegration(terminal, args)).toBe(args);
  });

  it("appends /K with the OSC 9;9 prompt for cmd", () => {
    const terminal = configForPreset("cmd");
    const args = withCwdIntegration(terminal, []);
    expect(args).toEqual(["/K", "prompt $E]9;9;$P$E\\$P$G"]);
  });

  it("leaves cmd args untouched when /k is already present (case-insensitive)", () => {
    const terminal = configForPreset("cmd");
    const args = ["/K", "dir"];
    expect(withCwdIntegration(terminal, args)).toBe(args);
  });

  it("builds the git-bash login script for the default --login -i args", () => {
    const terminal = configForPreset("git-bash");
    const args = withCwdIntegration(terminal, ["--login", "-i"]);
    expect(args[0]).toBe("-c");
    expect(args[1]).toContain("exec bash --login -i");
  });

  it("leaves git-bash args untouched for a non-default invocation", () => {
    const terminal = configForPreset("git-bash");
    const args = ["-c", "echo hi"];
    expect(withCwdIntegration(terminal, args)).toBe(args);
  });

  it("appends the wsl login script and preserves a distro arg", () => {
    const terminal = configForPreset("wsl");
    const args = withCwdIntegration(terminal, ["-d", "Ubuntu"]);
    expect(args.slice(0, 2)).toEqual(["-d", "Ubuntu"]);
    expect(args[2]).toBe("sh");
    expect(args[3]).toBe("-lc");
    expect(args[4]).toContain('exec "${SHELL:-bash}" -l');
  });

  it("leaves wsl args untouched when --exec is already present", () => {
    const terminal = configForPreset("wsl");
    const args = ["--exec", "bash"];
    expect(withCwdIntegration(terminal, args)).toBe(args);
  });

  it("never touches custom preset args", () => {
    const terminal = configForPreset("custom");
    const args = ["--anything"];
    expect(withCwdIntegration(terminal, args)).toBe(args);
  });
});

describe("withAgentLaunch", () => {
  function hooked(presetId: Parameters<typeof configForPreset>[0], seed: string[] = []) {
    const terminal = configForPreset(presetId);
    const argsForHook = [...seed];
    const hookedArgs = withCwdIntegration(terminal, argsForHook);
    return { terminal, argsForHook, hookedArgs };
  }

  it("chains the launch command onto the PowerShell -Command script with ;", () => {
    const { terminal, argsForHook, hookedArgs } = hooked("powershell");
    const args = withAgentLaunch(terminal, hookedArgs, argsForHook, "claude");
    expect(args[args.length - 1].endsWith("; claude --permission-mode auto")).toBe(true);
    // Only the trailing script argument changes; -NoExit/-Command are untouched.
    expect(args.slice(0, 2)).toEqual(hookedArgs.slice(0, 2));
  });

  it("chains the launch command onto the cmd /K string with &", () => {
    const { terminal, argsForHook, hookedArgs } = hooked("cmd");
    const args = withAgentLaunch(terminal, hookedArgs, argsForHook, "codex");
    expect(args).toEqual(["/K", "prompt $E]9;9;$P$E\\$P$G & codex --approve-for-me"]);
  });

  it("inserts the launch command before exec for git-bash, keeping the shell alive after", () => {
    const { terminal, argsForHook, hookedArgs } = hooked("git-bash", ["--login", "-i"]);
    const args = withAgentLaunch(terminal, hookedArgs, argsForHook, "claude");
    expect(args[1]).toContain("; claude --permission-mode auto; exec bash --login -i");
  });

  it("inserts the launch command before exec for wsl", () => {
    const { terminal, argsForHook, hookedArgs } = hooked("wsl");
    const args = withAgentLaunch(terminal, hookedArgs, argsForHook, "codex");
    expect(args[args.length - 1]).toContain('; codex --approve-for-me; exec "${SHELL:-bash}" -l');
  });

  it("is a no-op for the custom preset", () => {
    const { terminal, argsForHook, hookedArgs } = hooked("custom", ["--anything"]);
    const args = withAgentLaunch(terminal, hookedArgs, argsForHook, "claude");
    expect(args).toBe(hookedArgs);
    expect(args).not.toContain("claude");
  });

  it("does not guess at already-customized args that skipped the cwd hook", () => {
    const terminal = configForPreset("powershell");
    const argsForHook = ["-Command", "Get-Process"];
    const hookedArgs = withCwdIntegration(terminal, argsForHook); // returns argsForHook unchanged
    const args = withAgentLaunch(terminal, hookedArgs, argsForHook, "claude");
    expect(args).toBe(argsForHook);
    expect(args).not.toContain("claude");
  });
});

describe("macOS shell launch", () => {
  it("launches an account CLI and leaves a login shell after it exits", () => {
    const terminal = configForPreset("zsh");
    const args = terminal.args;
    expect(withAgentLaunch(terminal, withCwdIntegration(terminal, args), args, "claude"))
      .toEqual(["-l", "-i", "-c", "claude --permission-mode auto; exec /bin/zsh -l -i"]);
  });
  it("preserves an explicitly configured zsh command", () => {
    const terminal = configForPreset("zsh");
    const args = ["-c", "echo custom"];
    expect(withAgentLaunch(terminal, args, args, "claude")).toBe(args);
  });
});
