import { describe, it, expect } from "vitest";
import { buildNavigatorTree } from "./navigator";

describe("buildNavigatorTree", () => {
  it("carries each session's agent into the Navigator node", () => {
    const tree = buildNavigatorTree({
      workspaces: [{ id: "ws-a", name: "Alpha", index: 1 }],
      sessions: [{ id: "s1", name: "w1.assistant", agent: "codex" }],
      activeWorkspaceId: "ws-a",
      focusedSessionId: null,
    });

    expect(tree[0].sessions[0].agent).toBe("codex");
  });

  it("nests each session under the workspace named by its daemon-name prefix", () => {
    const tree = buildNavigatorTree({
      workspaces: [
        { id: "ws-a", name: "Alpha", index: 1 },
        { id: "ws-b", name: "Beta", index: 2 },
      ],
      sessions: [
        { id: "s1", name: "w1.session-1", agent: "terminal" },
        { id: "s2", name: "w2.build", agent: "terminal" },
        { id: "s3", name: "w1.logs", agent: "terminal" },
      ],
      activeWorkspaceId: "ws-a",
      focusedSessionId: null,
    });

    expect(tree.map((w) => w.id)).toEqual(["ws-a", "ws-b"]);
    expect(tree[0].sessions.map((s) => s.id)).toEqual(["s1", "s3"]);
    expect(tree[1].sessions.map((s) => s.id)).toEqual(["s2"]);
  });

  it("flags the active workspace and only that one", () => {
    const tree = buildNavigatorTree({
      workspaces: [
        { id: "ws-a", name: "Alpha", index: 1 },
        { id: "ws-b", name: "Beta", index: 2 },
      ],
      sessions: [],
      activeWorkspaceId: "ws-b",
      focusedSessionId: null,
    });

    expect(tree.map((w) => w.isActiveWorkspace)).toEqual([false, true]);
  });

  it("flags the focused session and exposes its un-prefixed display name", () => {
    const tree = buildNavigatorTree({
      workspaces: [{ id: "ws-a", name: "Alpha", index: 1 }],
      sessions: [
        { id: "s1", name: "w1.session-1", agent: "terminal" },
        { id: "s2", name: "w1.build", agent: "terminal" },
      ],
      activeWorkspaceId: "ws-a",
      focusedSessionId: "s2",
    });

    const [first, second] = tree[0].sessions;
    expect(first.isFocusedSession).toBe(false);
    expect(second.isFocusedSession).toBe(true);
    expect(first.displayName).toBe("session-1");
    expect(second.displayName).toBe("build");
  });

  it("lists a workspace with no sessions as an empty branch", () => {
    const tree = buildNavigatorTree({
      workspaces: [
        { id: "ws-a", name: "Alpha", index: 1 },
        { id: "ws-b", name: "Beta", index: 2 },
      ],
      sessions: [{ id: "s1", name: "w1.only", agent: "terminal" }],
      activeWorkspaceId: "ws-a",
      focusedSessionId: null,
    });

    expect(tree[1].id).toBe("ws-b");
    expect(tree[1].sessions).toEqual([]);
  });

  it("drops sessions with no prefix or a prefix matching no workspace", () => {
    const tree = buildNavigatorTree({
      workspaces: [{ id: "ws-a", name: "Alpha", index: 1 }],
      sessions: [
        { id: "s1", name: "w1.kept", agent: "terminal" },
        { id: "s2", name: "orphan-no-prefix", agent: "terminal" },
        { id: "s3", name: "w9.unmatched-index", agent: "terminal" },
      ],
      activeWorkspaceId: "ws-a",
      focusedSessionId: null,
    });

    const allSessionIds = tree.flatMap((w) => w.sessions.map((s) => s.id));
    expect(allSessionIds).toEqual(["s1"]);
  });

  it("returns an empty tree when there are no workspaces", () => {
    expect(
      buildNavigatorTree({
        workspaces: [],
        sessions: [{ id: "s1", name: "w1.session-1", agent: "terminal" }],
        activeWorkspaceId: null,
        focusedSessionId: null,
      }),
    ).toEqual([]);
  });

  it("flags each session's unseen activity from the activity map", () => {
    const tree = buildNavigatorTree({
      workspaces: [{ id: "ws-a", name: "Alpha", index: 1 }],
      sessions: [
        { id: "s1", name: "w1.plain", agent: "terminal" },
        { id: "s2", name: "w1.rang", agent: "terminal" },
        { id: "s3", name: "w1.quiet", agent: "terminal" },
      ],
      activeWorkspaceId: "ws-a",
      focusedSessionId: null,
      activityById: {
        s1: { output: true, bell: false },
        s2: { output: true, bell: true },
      },
    });

    const [s1, s2, s3] = tree[0].sessions;
    expect(s1.hasActivity).toBe(true);
    expect(s1.hasBell).toBe(false);
    expect(s2.hasActivity).toBe(true);
    expect(s2.hasBell).toBe(true);
    expect(s3.hasActivity).toBe(false);
    expect(s3.hasBell).toBe(false);
  });

  it("defaults activity flags to false when no activity map is given", () => {
    const tree = buildNavigatorTree({
      workspaces: [{ id: "ws-a", name: "Alpha", index: 1 }],
      sessions: [{ id: "s1", name: "w1.only", agent: "terminal" }],
      activeWorkspaceId: "ws-a",
      focusedSessionId: null,
    });

    const [only] = tree[0].sessions;
    expect(only.hasActivity).toBe(false);
    expect(only.hasBell).toBe(false);
  });
});
