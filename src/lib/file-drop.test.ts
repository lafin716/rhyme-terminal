import { describe, it, expect } from "vitest";
import { FILE_DRAG_MIME } from "./path-insert";
import {
  MAX_TEXT_BYTES,
  baseName,
  fileType,
  isExternalFileDrag,
  matchDroppedPaths,
  previewFromFile,
} from "./file-drop";

describe("isExternalFileDrag", () => {
  it("accepts an OS file drag", () => {
    expect(isExternalFileDrag(["Files"], false)).toBe(true);
  });

  it("ignores in-page drags and non-file drags", () => {
    expect(isExternalFileDrag(["Files"], true)).toBe(false);
    expect(isExternalFileDrag(["text/plain"], false)).toBe(false);
    expect(isExternalFileDrag(["Files", FILE_DRAG_MIME], false)).toBe(false);
    expect(isExternalFileDrag(undefined, false)).toBe(false);
  });
});

describe("baseName", () => {
  it("handles both separators", () => {
    expect(baseName("C:\\Users\\me\\a.png")).toBe("a.png");
    expect(baseName("/Users/me/b.txt")).toBe("b.txt");
  });
});

describe("matchDroppedPaths", () => {
  it("orders paths to match the dropped names", () => {
    expect(matchDroppedPaths(["b.txt", "a.png"], ["/x/a.png", "/y/b.txt"])).toEqual(["/y/b.txt", "/x/a.png"]);
  });

  it("pairs duplicate names with distinct paths", () => {
    expect(matchDroppedPaths(["a", "a"], ["/1/a", "/2/a"])).toEqual(["/1/a", "/2/a"]);
  });

  it("rejects a stale or partial path list", () => {
    expect(matchDroppedPaths(["new.png"], ["/old/old.png"])).toBeNull();
    expect(matchDroppedPaths(["a", "b"], ["/a"])).toBeNull();
    expect(matchDroppedPaths([], ["/a"])).toBeNull();
  });
});

describe("fileType", () => {
  it("maps extensions case-insensitively", () => {
    expect(fileType("Photo.JPG")).toEqual({ language: "image", mime: "image/jpeg" });
    expect(fileType("README.md")).toEqual({ language: "markdown", mime: "text/markdown" });
    expect(fileType("Makefile")).toEqual({ language: "plaintext", mime: "text/plain" });
    expect(fileType(".bashrc")).toEqual({ language: "plaintext", mime: "text/plain" });
  });
});

describe("previewFromFile", () => {
  it("ships image bytes as base64", async () => {
    const preview = await previewFromFile(new File([new Uint8Array([1, 2, 3])], "dot.png"));
    expect(preview).toMatchObject({ kind: "image", mime: "image/png", data: "AQID", canonicalPath: "", size: 3 });
  });

  it("reads UTF-8 text", async () => {
    const preview = await previewFromFile(new File(["# 제목\n"], "notes.md"));
    expect(preview).toMatchObject({ kind: "text", language: "markdown", text: "# 제목\n" });
  });

  it("marks undecodable content as binary", async () => {
    const preview = await previewFromFile(new File([new Uint8Array([0xff, 0xfe, 0x00])], "blob.dat"));
    expect(preview.kind).toBe("binary");
  });

  it("refuses text past the size cap", async () => {
    const preview = await previewFromFile(new File([new Uint8Array(MAX_TEXT_BYTES + 1)], "big.log"));
    expect(preview.kind).toBe("too_large");
  });
});
