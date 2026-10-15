import { describe, it, expect } from "vitest";
import {
  formatPathForInsertion,
  formatPathForShell,
  formatPathsForShell,
  parseDragPayload,
  shellQuoting,
  toDragPayload,
} from "./path-insert";

describe("formatPathForInsertion — plain paths pass through", () => {
  it("returns a Windows path without spaces unchanged", () => {
    expect(formatPathForInsertion("C:\\Users\\me\\notes.txt")).toBe(
      "C:\\Users\\me\\notes.txt",
    );
  });

  it("returns a POSIX path without spaces unchanged", () => {
    expect(formatPathForInsertion("/home/me/project/main.rs")).toBe(
      "/home/me/project/main.rs",
    );
  });

  it("returns a bare filename unchanged", () => {
    expect(formatPathForInsertion("README.md")).toBe("README.md");
  });
});

describe("formatPathForInsertion — spaced paths are quoted", () => {
  it("quotes a path whose directory contains a space", () => {
    expect(formatPathForInsertion("C:\\Program Files\\app\\run.exe")).toBe(
      '"C:\\Program Files\\app\\run.exe"',
    );
  });

  it("quotes a path whose filename contains a space", () => {
    expect(formatPathForInsertion("C:\\docs\\my report.pdf")).toBe(
      '"C:\\docs\\my report.pdf"',
    );
  });

  it("quotes when the path contains a tab (shell-significant whitespace)", () => {
    expect(formatPathForInsertion("C:\\a\tb.txt")).toBe('"C:\\a\tb.txt"');
  });
});

describe("formatPathForInsertion — edge inputs", () => {
  it("returns an empty string for empty input", () => {
    expect(formatPathForInsertion("")).toBe("");
  });
});

// The paths named in the Windows-integration test plan: a plain one, one with
// spaces, and one that is entirely Hangul with a space in it.
const PLAIN = "C:\\Users\\User\\Desktop\\hello.txt";
const SPACED = "C:\\Users\\User\\My Documents\\hello world.txt";
const HANGUL = "D:\\개발\\프로젝트\\테스트 파일.txt";

describe("formatPathForShell — PowerShell", () => {
  it("single-quotes every path, spaces or not", () => {
    expect(formatPathForShell(PLAIN, "windows-powershell")).toBe(`'${PLAIN}'`);
    expect(formatPathForShell(SPACED, "powershell")).toBe(`'${SPACED}'`);
  });

  it("keeps a Hangul path byte-for-byte", () => {
    expect(formatPathForShell(HANGUL, "windows-powershell")).toBe(`'${HANGUL}'`);
  });

  it("doubles an apostrophe rather than ending the string early", () => {
    expect(formatPathForShell("C:\\it's here\\a.txt", "powershell")).toBe(
      "'C:\\it''s here\\a.txt'",
    );
  });

  it("leaves $ and backticks literal, which single quotes already do", () => {
    expect(formatPathForShell("C:\\$env\\`x.txt", "powershell")).toBe(
      "'C:\\$env\\`x.txt'",
    );
  });
});

describe("formatPathForShell — cmd", () => {
  it("double-quotes, the only quoting cmd has", () => {
    expect(formatPathForShell(SPACED, "cmd")).toBe(`"${SPACED}"`);
    expect(formatPathForShell(HANGUL, "cmd")).toBe(`"${HANGUL}"`);
  });

  it("keeps & and ^ inside the quotes instead of escaping them", () => {
    expect(formatPathForShell("C:\\a&b^c\\x.txt", "cmd")).toBe('"C:\\a&b^c\\x.txt"');
  });
});

describe("formatPathForShell — POSIX shells", () => {
  it("rewrites a Windows drive to /mnt for WSL, where C:\\ does not exist", () => {
    expect(formatPathForShell(SPACED, "wsl")).toBe(
      "'/mnt/c/Users/User/My Documents/hello world.txt'",
    );
    expect(formatPathForShell(HANGUL, "wsl")).toBe(
      "'/mnt/d/개발/프로젝트/테스트 파일.txt'",
    );
  });

  it("only swaps separators for Git Bash, which resolves C:/ but not C:\\", () => {
    expect(formatPathForShell(SPACED, "git-bash")).toBe(
      "'C:/Users/User/My Documents/hello world.txt'",
    );
  });

  it("escapes an apostrophe by closing, escaping and reopening", () => {
    expect(formatPathForShell("/home/me/it's.txt", "zsh")).toBe(
      "'/home/me/it'\\''s.txt'",
    );
  });

  it("leaves $ and spaces literal inside single quotes", () => {
    expect(formatPathForShell("/home/me/$PATH x.txt", "zsh")).toBe(
      "'/home/me/$PATH x.txt'",
    );
  });
});

describe("formatPathsForShell — multiple paths in one drop", () => {
  it("joins them with spaces, each quoted on its own", () => {
    expect(
      formatPathsForShell(["C:\\a.txt", "C:\\b.txt", "C:\\My Files\\c.txt"], "powershell"),
    ).toBe("'C:\\a.txt' 'C:\\b.txt' 'C:\\My Files\\c.txt'");
  });

  it("converts every path for the target shell", () => {
    expect(formatPathsForShell(["C:\\a.txt", "D:\\b.txt"], "wsl")).toBe(
      "'/mnt/c/a.txt' '/mnt/d/b.txt'",
    );
  });

  it("drops blanks so a trailing newline cannot add an empty argument", () => {
    expect(formatPathsForShell(["C:\\a.txt", "  ", ""], "cmd")).toBe('"C:\\a.txt"');
  });

  it("is empty for an empty drop", () => {
    expect(formatPathsForShell([], "powershell")).toBe("");
  });
});

describe("drag payload — one path per line", () => {
  it("round-trips several paths", () => {
    const paths = [PLAIN, SPACED, HANGUL];
    expect(parseDragPayload(toDragPayload(paths))).toEqual(paths);
  });

  it("reads a single-path payload, the shape the Explorer panel has always sent", () => {
    expect(parseDragPayload(PLAIN)).toEqual([PLAIN]);
  });

  it("ignores blank lines", () => {
    expect(parseDragPayload("C:\\a.txt\n\nC:\\b.txt\n")).toEqual([
      "C:\\a.txt",
      "C:\\b.txt",
    ]);
  });
});

describe("shellQuoting — custom presets guess from the program", () => {
  it("recognises the shells a custom preset is most likely to name", () => {
    expect(shellQuoting("custom", "C:\\Windows\\System32\\cmd.exe")).toBe("cmd");
    expect(shellQuoting("custom", "C:\\Program Files\\PowerShell\\7\\pwsh.exe")).toBe(
      "powershell",
    );
    expect(shellQuoting("custom", "C:\\Program Files\\Git\\bin\\bash.exe")).toBe("posix");
    expect(shellQuoting("custom", "/bin/zsh")).toBe("posix");
  });

  it("falls back to PowerShell quoting when the program says nothing", () => {
    expect(shellQuoting("custom", "")).toBe("powershell");
  });
});
