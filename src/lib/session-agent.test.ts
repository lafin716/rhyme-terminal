import { describe, expect, it } from "vitest";
import { applySessionAgent, applySessionAgentUpdates } from "./session-agent";

describe("applySessionAgent", () => {
  it("updates only the matching session's agent", () => {
    const first = { id: "s1", agent: "terminal" as const, name: "shell" };
    const second = { id: "s2", agent: "claude" as const, name: "assistant" };

    const next = applySessionAgent([first, second], { id: "s1", agent: "codex" });

    expect(next).toEqual([
      { id: "s1", agent: "codex", name: "shell" },
      second,
    ]);
    expect(next[0]).not.toBe(first);
    expect(next[1]).toBe(second);
  });

  it.each([
    [{ id: "missing", agent: "codex" }],
    [{ id: "s1", agent: "terminal" }],
  ] as const)("preserves the array when an update changes nothing", (update) => {
    const sessions = [{ id: "s1", agent: "terminal" as const }];

    expect(applySessionAgent(sessions, update)).toBe(sessions);
  });

  it("replays an agent event that arrived after a list snapshot began", () => {
    const snapshot = [{ id: "s1", agent: "terminal" as const }];

    const next = applySessionAgentUpdates(snapshot, [
      { revision: 2, payload: { id: "s1", agent: "codex" as const } },
    ], 1);

    expect(next).toEqual([{ id: "s1", agent: "codex" }]);
  });

  it("replays an agent event that arrived before a create response", () => {
    const created = [{ id: "s-new", agent: "terminal" as const }];

    const next = applySessionAgentUpdates(created, [
      { revision: 4, payload: { id: "s-new", agent: "claude" as const } },
    ], 3);

    expect(next).toEqual([{ id: "s-new", agent: "claude" }]);
  });
});
