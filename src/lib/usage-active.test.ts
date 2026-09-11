import { describe, expect, it } from "vitest";
import { activeUsageProfiles, parseUsageKey } from "./usage-active";
import type { SessionProfile } from "./session-profile";

const profiles: Record<string, SessionProfile> = {
  "s-work": { agent: "claude", id: "work", label: "Work" },
  "s-system": { agent: "codex", id: null, label: "" },
  "s-stale": { agent: "claude", id: "old", label: "Old" },
};
const profileOf = (id: string) => profiles[id];

describe("active usage profiles", () => {
  it("only reports profiles whose PTY is running a CLI agent", () => {
    expect(activeUsageProfiles([
      { id: "s-work", agent: "claude" },
      { id: "s-system", agent: "codex" },
      { id: "s-idle", agent: "terminal" },
    ], profileOf)).toEqual([
      { agent: "claude", id: "work" },
      { agent: "codex", id: null },
    ]);
    expect(activeUsageProfiles([{ id: "s-work", agent: "terminal" }], profileOf)).toEqual([]);
  });

  it("attributes an agent with no recorded launch profile to the system login", () => {
    expect(activeUsageProfiles([{ id: "unknown", agent: "claude" }], profileOf))
      .toEqual([{ agent: "claude", id: null }]);
    // A snapshot left over from a different agent must not be trusted.
    expect(activeUsageProfiles([{ id: "s-stale", agent: "codex" }], profileOf))
      .toEqual([{ agent: "codex", id: null }]);
  });

  it("takes a loop group's profile from the group, never from its managed PTY", () => {
    const loops = [{ activeProfile: "claude:loop", live: true, sessionIds: ["s-loop", "s-old"] }];
    expect(activeUsageProfiles([
      { id: "s-loop", agent: "claude" },
      { id: "s-old", agent: "claude" },
      { id: "s-work", agent: "claude" },
    ], profileOf, loops)).toEqual([
      { agent: "claude", id: "loop" },
      { agent: "claude", id: "work" },
    ]);
  });

  it("ignores a loop with no live agent and de-duplicates shared profiles", () => {
    expect(activeUsageProfiles([], profileOf, [
      { activeProfile: "claude:loop", live: false, sessionIds: ["s-loop"] },
      { activeProfile: null, live: true, sessionIds: [] },
    ])).toEqual([]);
    expect(activeUsageProfiles(
      [{ id: "s-work", agent: "claude" }],
      profileOf,
      [{ activeProfile: "claude:work", live: true, sessionIds: ["s-loop"] }],
    )).toEqual([{ agent: "claude", id: "work" }]);
  });

  it("parses usage keys and rejects malformed ones", () => {
    expect(parseUsageKey("claude:system")).toEqual({ agent: "claude", id: null });
    expect(parseUsageKey("codex:abc")).toEqual({ agent: "codex", id: "abc" });
    expect(parseUsageKey("terminal:abc")).toBeUndefined();
    expect(parseUsageKey("claude")).toBeUndefined();
    expect(parseUsageKey("claude:")).toBeUndefined();
  });
});
