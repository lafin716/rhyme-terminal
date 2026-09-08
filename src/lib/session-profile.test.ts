import { describe, expect, it } from "vitest";
import { profileForLaunch } from "./session-profile";
import type { AccountProfile } from "./persistence";

const profiles: AccountProfile[] = [
  { id: "personal", agent: "claude", label: "Personal", configDir: "C:\\claude", createdAt: 0 },
  { id: "work", agent: "codex", label: "Work", configDir: "C:\\codex", createdAt: 0 },
];

describe("session profile identity", () => {
  it("records the selected profile without persisting credentials", () => {
    expect(profileForLaunch(profiles, { CLAUDE_CONFIG_DIR: "C:\\claude", CLAUDE_CODE_OAUTH_TOKEN: "secret" }, "claude"))
      .toEqual({ agent: "claude", id: "personal", label: "Personal" });
  });
  it("uses the launched agent when both profile environments are present", () => {
    expect(profileForLaunch(profiles, { CLAUDE_CONFIG_DIR: "C:\\claude", CODEX_HOME: "C:\\codex" }, "codex")?.id).toBe("work");
  });
  it("distinguishes system agents, plain terminals and unknown profile folders", () => {
    expect(profileForLaunch(profiles, undefined, "codex")).toEqual({ agent: "codex", id: null, label: "" });
    expect(profileForLaunch(profiles)).toBeUndefined();
    expect(profileForLaunch(profiles, { CODEX_HOME: "C:\\unknown" }, "codex")).toBeUndefined();
  });
});
