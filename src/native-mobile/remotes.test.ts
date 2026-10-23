import { describe, expect, it } from "vitest";
import {
  MAX_REMOTES, REMOTES_KEY, connectWithInvitation, loadRemotes, remoteTarget, removeRemote, renameRemote, saveRemotes, touchRemote,
  type RemoteEntry,
} from "./remotes";

function memory(initial?: string) {
  const data = new Map<string, string>();
  if (initial !== undefined) data.set(REMOTES_KEY, initial);
  return { data, getItem: (key: string) => data.get(key) ?? null, setItem: (key: string, value: string) => { data.set(key, value); } };
}

const office: RemoteEntry = { origin: "http://100.70.1.2:43123", name: "사무실", lastConnectedAt: 10 };
const home: RemoteEntry = { origin: "http://100.80.0.9:43123", name: "집", lastConnectedAt: 20 };

describe("saved remotes", () => {
  it("round-trips the versioned envelope without secrets", () => {
    const storage = memory();
    saveRemotes(storage, [office]);
    expect(JSON.parse(storage.data.get(REMOTES_KEY)!)).toEqual({ version: 1, remotes: [office] });
    expect(loadRemotes(storage)).toEqual([office]);
  });

  it.each(["", "not json", "[]", JSON.stringify({ version: 2, remotes: [office] }), JSON.stringify({ version: 1, remotes: "x" })])(
    "treats %j as an empty list", raw => {
      expect(loadRemotes(memory(raw))).toEqual([]);
    });

  it("drops invalid, unsafe and duplicate entries and sorts by last connection", () => {
    const raw = JSON.stringify({ version: 1, remotes: [
      office,
      { origin: "javascript:alert(1)", name: "x", lastConnectedAt: null },
      { origin: "http://100.70.1.2:43123/path", name: "x", lastConnectedAt: null },
      { origin: "http://u:p@host", name: "x", lastConnectedAt: null },
      { origin: office.origin, name: "dup", lastConnectedAt: 99 },
      { ...home, token: "must-not-survive" },
      { origin: "http://host", name: "   ", lastConnectedAt: null },
    ] });
    expect(loadRemotes(memory(raw))).toEqual([home, office, { origin: "http://host", name: "host", lastConnectedAt: null }]);
  });

  it("adds a gateway from an invite and navigates with the invite", () => {
    const { remotes, target } = connectWithInvitation([], " http://100.70.1.2:43123/#invite=abc ", 5);
    expect(remotes).toEqual([{ origin: "http://100.70.1.2:43123", name: "100.70.1.2:43123", lastConnectedAt: 5 }]);
    expect(new URLSearchParams(new URL(target).hash.slice(1)).get("invite")).toBe("abc");
  });

  it("updates an existing gateway instead of duplicating it and keeps its name", () => {
    const { remotes } = connectWithInvitation([office, home], "http://100.70.1.2:43123/#invite=new", 30);
    expect(remotes).toEqual([{ ...office, lastConnectedAt: 30 }, home]);
  });

  it("rejects invalid invites and a full list", () => {
    expect(() => connectWithInvitation([], "http://host/", 1)).toThrow();
    const full = Array.from({ length: MAX_REMOTES }, (_, i) => ({ origin: `http://10.0.0.${i}`, name: `${i}`, lastConnectedAt: null }));
    expect(() => connectWithInvitation(full, "http://10.1.1.1/#invite=a", 1)).toThrow(/최대/);
    expect(connectWithInvitation(full, "http://10.0.0.3/#invite=a", 1).remotes).toHaveLength(MAX_REMOTES);
  });

  it("reconnects to the saved origin with only the native marker", () => {
    const url = new URL(remoteTarget(office.origin));
    expect(url.origin).toBe(office.origin);
    expect(url.pathname).toBe("/");
    expect(url.hash).toBe("#native=android");
  });

  it("renames, touches and removes by origin", () => {
    expect(renameRemote([office], office.origin, "  새   이름 ")[0].name).toBe("새 이름");
    expect(renameRemote([office], office.origin, "")[0].name).toBe("100.70.1.2:43123");
    expect(renameRemote([office], office.origin, "x".repeat(80))[0].name).toHaveLength(40);
    expect(touchRemote([home, office], office.origin, 50).map(entry => entry.origin)).toEqual([office.origin, home.origin]);
    expect(removeRemote([home, office], home.origin)).toEqual([office]);
  });
});
