import { describe, expect, it } from "vitest";
import { claudeIcon, codexIcon, terminalIcon } from "./offline-icons";
import { sessionAgentIcon } from "./session-agent-icon";

describe("sessionAgentIcon", () => {
  it.each([
    ["terminal", terminalIcon],
    ["claude", claudeIcon],
    ["codex", codexIcon],
  ] as const)("maps %s sessions to their local icon", (agent, expected) => {
    expect(sessionAgentIcon(agent)).toBe(expected);
  });
});
