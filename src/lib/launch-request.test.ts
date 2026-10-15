import { describe, it, expect } from "vitest";
import { isActionableLaunch, launchSessionOptions } from "./launch-request";

describe("launchSessionOptions", () => {
  it("opens a session at the directory the request named", () => {
    expect(launchSessionOptions({ cwd: "D:\\workspace\\project" })).toEqual({
      cwd: "D:\\workspace\\project",
      paths: [],
    });
  });

  it("runs the command a caller spelled out", () => {
    expect(launchSessionOptions({ cwd: "D:\\p", command: "git status" })).toEqual({
      cwd: "D:\\p",
      launchCommand: "git status",
      paths: [],
    });
  });

  it("turns an ssh target into an ssh command, so the app decides how to connect", () => {
    expect(launchSessionOptions({ ssh: "user@host" })).toEqual({
      launchCommand: "ssh user@host",
      paths: [],
    });
  });

  it("prefers an explicit command over an ssh target", () => {
    expect(launchSessionOptions({ ssh: "user@host", command: "ssh -p 2222 user@host" }))
      .toEqual({ launchCommand: "ssh -p 2222 user@host", paths: [] });
  });

  it("keeps the files a request named, for the prompt rather than for execution", () => {
    const files = ["D:\\p\\README.md", "D:\\p\\주요 문서.txt"];
    expect(launchSessionOptions({ cwd: "D:\\p", files }).paths).toEqual(files);
  });

  it("ignores a command that is only whitespace", () => {
    expect(launchSessionOptions({ cwd: "D:\\p", command: "   " })).toEqual({
      cwd: "D:\\p",
      paths: [],
    });
  });

  it("asks for nothing in particular when the request is empty", () => {
    expect(launchSessionOptions({})).toEqual({ paths: [] });
  });
});

describe("isActionableLaunch", () => {
  it("is false for a request that only means 'show the window'", () => {
    expect(isActionableLaunch({})).toBe(false);
    expect(isActionableLaunch({ command: "  ", newWindow: true })).toBe(false);
  });

  it("is true as soon as the request names a place, a command or a file", () => {
    expect(isActionableLaunch({ cwd: "D:\\p" })).toBe(true);
    expect(isActionableLaunch({ command: "git status" })).toBe(true);
    expect(isActionableLaunch({ ssh: "user@host" })).toBe(true);
    expect(isActionableLaunch({ files: ["D:\\p\\a.txt"] })).toBe(true);
  });
});
