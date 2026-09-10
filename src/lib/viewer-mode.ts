// Pure resolver for the FileViewer: given a file's preview metadata, decide
// which view the component should render. `FileViewer.vue` switches on this
// output instead of branching inline on `preview.kind`, so the routing rules
// live in one headless, unit-tested place (the `navigator.ts`/`quick-open.ts`
// seam style — no Vue/DOM/Tauri dependency).
//
// Rules: binary/too_large/image/pdf come straight from the backend `kind`; a
// text preview whose language is `markdown` renders as formatted Markdown, and
// every other text preview stays in the Monaco text view.

import type { FilePreview } from "./tauri";

export type ViewerMode =
  | "markdown"
  | "pdf"
  | "image"
  | "text"
  | "binary"
  | "too_large";

/** The narrow slice of a `FilePreview` the resolver actually reads. */
export type ViewerModeInput = Pick<FilePreview, "kind" | "language">;

/**
 * Whether a mode has a raw source view behind it. Markdown is the only one: the
 * backend ships it as text, so the rendered page can always be swapped for the
 * editable source. Image/PDF/binary have no text to edit, and `text` is already
 * its own source.
 */
export function canShowSource(mode: ViewerMode): boolean {
  return mode === "markdown";
}

/**
 * The view the FileViewer actually renders, once the Markdown source toggle is
 * applied. Toggling source on Markdown drops to the editable `text` view;
 * everything else ignores the flag.
 */
export function effectiveViewerMode(mode: ViewerMode, showSource: boolean): ViewerMode {
  return showSource && canShowSource(mode) ? "text" : mode;
}

export function resolveViewerMode(preview: ViewerModeInput): ViewerMode {
  switch (preview.kind) {
    case "binary":
      return "binary";
    case "too_large":
      return "too_large";
    case "image":
      return "image";
    case "pdf":
      return "pdf";
    case "text":
      return preview.language === "markdown" ? "markdown" : "text";
  }
}
