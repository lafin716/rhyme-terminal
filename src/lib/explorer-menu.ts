// Headless model for the Explorer's right-click menu: which commands a row
// offers, whether a typed name is a legal leaf name, and how a path reads
// relative to the tree root. `ExplorerPanel.vue` renders whatever this returns
// and `ExplorerContextMenu.vue` only draws it, so the rules live in one
// unit-testable place with no Vue/DOM/Tauri dependency (the `viewer-mode.ts` /
// `quick-open.ts` seam style).

export type ExplorerAction =
  | "open"
  | "newFile"
  | "newFolder"
  | "rename"
  | "delete"
  | "copyPath"
  | "copyRelativePath"
  | "reveal"
  | "refresh";

export interface ExplorerMenuItem {
  action: ExplorerAction;
  /** English source message, translated at render time. */
  label: string;
  /** Draws a divider above this item. */
  divider?: boolean;
  /** Styled as destructive. */
  danger?: boolean;
}

export interface ExplorerMenuTarget {
  isDir: boolean;
  /**
   * The tree root itself (right-clicking empty space below the rows). It has no
   * parent inside the tree, so it offers neither rename nor delete.
   */
  isRoot: boolean;
}

/**
 * The context-menu commands for one Explorer row, in display order. Files lead
 * with Open; directories lead with the two New commands (which always create
 * *inside* a directory, and beside a file). The root drops the destructive and
 * relative-path commands.
 */
export function explorerMenuItems(target: ExplorerMenuTarget): ExplorerMenuItem[] {
  const items: ExplorerMenuItem[] = [];

  if (!target.isDir) items.push({ action: "open", label: "Open" });
  items.push({ action: "newFile", label: "New File", divider: !target.isDir });
  items.push({ action: "newFolder", label: "New Folder" });

  items.push({ action: "copyPath", label: "Copy Path", divider: true });
  if (!target.isRoot) items.push({ action: "copyRelativePath", label: "Copy Relative Path" });

  items.push({ action: "reveal", label: "Reveal in File Explorer", divider: true });
  if (target.isDir) items.push({ action: "refresh", label: "Refresh" });

  if (!target.isRoot) {
    items.push({ action: "rename", label: "Rename", divider: true });
    items.push({ action: "delete", label: "Delete", danger: true });
  }
  return items;
}

/** Path separators and the characters Windows forbids in a file name. */
const INVALID_NAME_CHARS = /[<>:"|?*/\\]/;

/**
 * Rejects a typed name that is not a plain leaf name, returning the English
 * source message to show under the inline editor (or `null` when it is fine).
 * Mirrors the backend's `validate_entry_name` so the common mistakes are caught
 * before the round trip — the backend stays the authority.
 */
export function validateEntryName(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return "A name is required.";
  if (trimmed === "." || trimmed === "..") return "That name is reserved.";
  if (INVALID_NAME_CHARS.test(trimmed)) return "A name cannot contain \\ / : * ? \" < > |";
  if ([...trimmed].some((c) => c.charCodeAt(0) < 0x20)) {
    return "A name cannot contain control characters.";
  }
  return null;
}

/**
 * `path` as it reads under the tree root, always `/`-joined — what "Copy
 * Relative Path" puts on the clipboard. Falls back to the absolute path when
 * the entry is not under the root (comparison is case-insensitive, since the
 * tree is rooted at a Windows path).
 */
export function relativeDisplayPath(root: string, path: string): string {
  const normalize = (value: string) => value.replace(/[\\/]+/g, "/").replace(/\/+$/, "");
  const base = normalize(root);
  const full = normalize(path);
  if (!base || !full.toLocaleLowerCase().startsWith(`${base.toLocaleLowerCase()}/`)) {
    return path;
  }
  return full.slice(base.length + 1);
}
