import { describe, expect, it } from "vitest";
import {
  explorerMenuItems,
  relativeDisplayPath,
  validateEntryName,
  type ExplorerAction,
} from "./explorer-menu";

const actions = (target: { isDir: boolean; isRoot: boolean }): ExplorerAction[] =>
  explorerMenuItems(target).map((item) => item.action);

describe("explorerMenuItems", () => {
  it("leads a file with Open and a directory with the New commands", () => {
    expect(actions({ isDir: false, isRoot: false })[0]).toBe("open");
    expect(actions({ isDir: true, isRoot: false })[0]).toBe("newFile");
    expect(actions({ isDir: true, isRoot: false })).not.toContain("open");
    // Refresh only makes sense for something with children to re-read.
    expect(actions({ isDir: false, isRoot: false })).not.toContain("refresh");
    expect(actions({ isDir: true, isRoot: false })).toContain("refresh");
  });

  it("withholds the destructive commands from the tree root", () => {
    const root = actions({ isDir: true, isRoot: true });
    expect(root).not.toContain("rename");
    expect(root).not.toContain("delete");
    expect(root).not.toContain("copyRelativePath");
    expect(root).toEqual(["newFile", "newFolder", "copyPath", "reveal", "refresh"]);
  });

  it("offers rename and delete last, with delete marked destructive", () => {
    const items = explorerMenuItems({ isDir: false, isRoot: false });
    expect(items[items.length - 2].action).toBe("rename");
    expect(items[items.length - 1]).toMatchObject({ action: "delete", danger: true });
    expect(items.every((item) => item.label.length > 0)).toBe(true);
  });
});

describe("validateEntryName", () => {
  it("accepts plain leaf names and trims them", () => {
    for (const name of ["notes.md", ".gitignore", "with space.txt", "한글.txt"]) {
      expect(validateEntryName(name), name).toBeNull();
    }
    expect(validateEntryName("  notes.md  ")).toBeNull();
  });

  it("rejects anything that is not a sibling name", () => {
    for (const name of ["", "   ", ".", "..", "a/b", "a\\b", "c:x", "a*", "a?", 'a"', "a|", "a<b"]) {
      expect(validateEntryName(name), name).not.toBeNull();
    }
    expect(validateEntryName("bell\u0007")).toBe("A name cannot contain control characters.");
  });
});

describe("relativeDisplayPath", () => {
  it("renders a path under the root as a `/`-joined relative path", () => {
    expect(relativeDisplayPath("C:\\repo", "C:\\repo\\src\\lib\\a.ts")).toBe("src/lib/a.ts");
    expect(relativeDisplayPath("C:\\repo\\", "C:\\Repo\\src\\a.ts")).toBe("src/a.ts");
    expect(relativeDisplayPath("/home/u/repo", "/home/u/repo/src/a.ts")).toBe("src/a.ts");
  });

  it("falls back to the absolute path when the entry is outside the root", () => {
    expect(relativeDisplayPath("C:\\repo", "C:\\other\\a.ts")).toBe("C:\\other\\a.ts");
    expect(relativeDisplayPath("", "C:\\repo\\a.ts")).toBe("C:\\repo\\a.ts");
    // The root itself is not "under" the root; it has no relative form.
    expect(relativeDisplayPath("C:\\repo", "C:\\repo")).toBe("C:\\repo");
  });
});
