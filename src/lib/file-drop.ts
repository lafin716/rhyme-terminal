// Helpers for files dropped onto the window from the OS file manager. The
// window keeps native drag-drop off (pane tabs need HTML5 drag-and-drop), so a
// drop arrives as DOM `File` objects; the real path is recovered per platform
// (see `src-tauri/src/file_drop.rs`) and these helpers glue the pieces together.

import { bytesToBase64, type FilePreview } from "./tauri";
import { FILE_DRAG_MIME } from "./path-insert";

/** Prefix of the WebView2 web message that carries dropped `File` objects. */
export const DROP_MESSAGE_PREFIX = "rhyme-file-drop:";

/** Same caps as the backend preview: text up to 5 MB, images/PDFs up to 32 MB. */
export const MAX_TEXT_BYTES = 5 * 1024 * 1024;
export const MAX_MEDIA_BYTES = 32 * 1024 * 1024;

/**
 * True for a drag that carries files from outside the page. In-page drags
 * (tabs, Explorer rows) are excluded by the caller's `internal` flag and by the
 * Explorer's own MIME type.
 */
export function isExternalFileDrag(types: readonly string[] | null | undefined, internal: boolean): boolean {
  if (internal || !types) return false;
  return types.includes("Files") && !types.includes(FILE_DRAG_MIME);
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/**
 * Pairs each dropped file name with a recovered path. Returns `null` unless
 * every name is accounted for, which guards against reading a stale drag
 * pasteboard left over from an earlier drag.
 */
export function matchDroppedPaths(names: readonly string[], paths: readonly string[]): string[] | null {
  if (names.length === 0 || paths.length < names.length) return null;
  const remaining = [...paths];
  const matched: string[] = [];
  for (const name of names) {
    const index = remaining.findIndex((path) => baseName(path) === name);
    if (index < 0) return null;
    matched.push(remaining[index]);
    remaining.splice(index, 1);
  }
  return matched;
}

const TYPES: Record<string, [language: string, mime: string]> = {
  rs: ["rust", "text/plain"],
  ts: ["typescript", "text/plain"],
  tsx: ["typescript", "text/plain"],
  js: ["javascript", "text/plain"],
  jsx: ["javascript", "text/plain"],
  mjs: ["javascript", "text/plain"],
  cjs: ["javascript", "text/plain"],
  vue: ["vue", "text/plain"],
  json: ["json", "application/json"],
  toml: ["toml", "text/plain"],
  yaml: ["yaml", "text/plain"],
  yml: ["yaml", "text/plain"],
  md: ["markdown", "text/markdown"],
  pdf: ["pdf", "application/pdf"],
  html: ["html", "text/html"],
  htm: ["html", "text/html"],
  css: ["css", "text/css"],
  scss: ["css", "text/css"],
  less: ["css", "text/css"],
  py: ["python", "text/plain"],
  sh: ["shell", "text/plain"],
  bash: ["shell", "text/plain"],
  ps1: ["powershell", "text/plain"],
  cmd: ["batch", "text/plain"],
  bat: ["batch", "text/plain"],
  c: ["c", "text/plain"],
  h: ["c", "text/plain"],
  cpp: ["cpp", "text/plain"],
  cc: ["cpp", "text/plain"],
  cxx: ["cpp", "text/plain"],
  hpp: ["cpp", "text/plain"],
  png: ["image", "image/png"],
  jpg: ["image", "image/jpeg"],
  jpeg: ["image", "image/jpeg"],
  jfif: ["image", "image/jpeg"],
  gif: ["image", "image/gif"],
  webp: ["image", "image/webp"],
  bmp: ["image", "image/bmp"],
  svg: ["image", "image/svg+xml"],
  ico: ["image", "image/x-icon"],
  avif: ["image", "image/avif"],
};

/** Language and MIME type by extension, mirroring the backend's `file_type`. */
export function fileType(name: string): { language: string; mime: string } {
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  const [language, mime] = TYPES[ext] ?? ["plaintext", "text/plain"];
  return { language, mime };
}

/**
 * Builds a preview from the dropped file's contents, for when its path cannot
 * be recovered. The result has no `canonicalPath`; saving goes through Save As.
 */
export async function previewFromFile(file: File): Promise<FilePreview> {
  const { language, mime } = fileType(file.name);
  const base = { canonicalPath: "", name: file.name, language, mime, size: file.size };
  const media = mime.startsWith("image/") || mime === "application/pdf";
  if (file.size > (media ? MAX_MEDIA_BYTES : MAX_TEXT_BYTES)) {
    return { ...base, kind: "too_large" };
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (media) {
    return { ...base, kind: mime === "application/pdf" ? "pdf" : "image", data: bytesToBase64(bytes) };
  }
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    if (text.includes("\0")) return { ...base, kind: "binary" };
    return { ...base, kind: "text", text };
  } catch {
    return { ...base, kind: "binary" };
  }
}
