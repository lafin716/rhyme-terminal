import type { LayoutNode, TerminalTabSnapshot, WorkspaceStore } from "./layout-types";
import type { Keybinding } from "./keybindings";
import type { TerminalConfig } from "./terminal-config";
import type { ShellPanelsState } from "./shell-panels";
import type { Locale } from "./i18n";

const STORAGE_KEY = "winmux:workspaces:v1";
const KEYBINDINGS_KEY = "winmux:keybindings:v1";
const PREFS_KEY = "winmux:prefs:v1";
const PALETTE_KEY = "winmux:palette:v1";
const ACCOUNTS_KEY = "winmux:accounts:v1";

interface Persisted {
  version: 1;
  store: WorkspaceStore;
}

interface PersistedKeybindings {
  version: 1;
  bindings: Record<string, Keybinding>;
}

export type PaletteUiMode = "context" | "radial";

export interface Prefs {
  sessionMenuOrder?: string[];
  language: Locale;
  showAccountProfile: boolean;
  skipKillSessionConfirm: boolean;
  defaultTerminal: TerminalConfig;
  paletteUiMode: PaletteUiMode;
  panels: ShellPanelsState;
  /**
   * Per-agent account profile used as that agent's default login — e.g. by
   * the "Launch Claude" quick action when no profile is picked explicitly.
   * `null` means the system account (no `CLAUDE_CONFIG_DIR`/`CODEX_HOME`
   * override) — see `useAccountProfiles.ts` and `resolveDefaultProfile` in
   * `default-profile.ts`.
   */
  defaultProfileId: Record<CliAgentKind, string | null>;
  toolbarProfileId?: Partial<Record<CliAgentKind, string | null>>;
  systemAccountEnv?: Partial<Record<CliAgentKind, Record<string, string>>>;
}

/**
 * Prefs as they may appear on disk, including fields written by older versions
 * (e.g. the legacy `sidebarMode` replaced by `panels`). Loaded prefs are
 * normalized/migrated into the strict {@link Prefs} shape on read.
 */
export type StoredPrefs = Partial<Prefs> & {
  sidebarMode?: string;
};

interface PersistedPrefs {
  version: 1;
  prefs: StoredPrefs;
}

export function loadWorkspaces(): WorkspaceStore | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Persisted;
    if (parsed.version !== 1 || !parsed.store) return null;
    return parsed.store;
  } catch (e) {
    console.warn("loadWorkspaces failed", e);
    return null;
  }
}

export function saveWorkspaces(store: WorkspaceStore): void {
  try {
    const cleanStore: WorkspaceStore = {
      activeWorkspaceId: store.activeWorkspaceId,
      workspaces: store.workspaces.map((workspace) => ({
        ...workspace,
        settings: {
          ...workspace.settings,
          terminal: workspace.settings.terminal
            ? {
                ...workspace.settings.terminal,
                args: [...workspace.settings.terminal.args],
              }
            : null,
        },
        layout: withoutTransientTabs(workspace.layout),
        terminalSnapshots: terminalSnapshotsForLayout(
          workspace.terminalSnapshots ?? {},
          workspace.layout,
        ),
      })),
    };
    const payload: Persisted = { version: 1, store: cleanStore };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
  } catch (e) {
    console.warn("saveWorkspaces failed", e);
  }
}

function terminalSnapshotsForLayout(
  snapshots: Record<string, TerminalTabSnapshot>,
  layout: LayoutNode,
): Record<string, TerminalTabSnapshot> {
  const ids = new Set(collectPersistentTabIds(layout));
  const out: Record<string, TerminalTabSnapshot> = {};
  for (const [id, snapshot] of Object.entries(snapshots)) {
    if (!ids.has(id)) continue;
    out[id] = {
      accountProfile: snapshot.accountProfile,
      name: snapshot.name,
      cwd: snapshot.cwd ?? null,
      terminal: {
        ...snapshot.terminal,
        args: [...snapshot.terminal.args],
      },
    };
  }
  return out;
}

function collectPersistentTabIds(node: LayoutNode): string[] {
  if (node.kind === "leaf") return node.tabs.filter((id) => !id.startsWith("resource-"));
  const out: string[] = [];
  for (const child of node.children) out.push(...collectPersistentTabIds(child));
  return out;
}

function withoutTransientTabs(node: LayoutNode): LayoutNode {
  if (node.kind === "leaf") {
    const tabs = node.tabs.filter((id) => !id.startsWith("resource-"));
    return {
      ...node,
      tabs,
      activeTabId: node.activeTabId && tabs.includes(node.activeTabId)
        ? node.activeTabId
        : (tabs[0] ?? null),
    };
  }
  return {
    ...node,
    sizes: [...node.sizes],
    children: node.children.map(withoutTransientTabs),
  };
}

export function loadKeybindings(): Record<string, Keybinding> | null {
  try {
    const raw = localStorage.getItem(KEYBINDINGS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedKeybindings;
    if (parsed.version !== 1 || !parsed.bindings) return null;
    return parsed.bindings;
  } catch (e) {
    console.warn("loadKeybindings failed", e);
    return null;
  }
}

export function saveKeybindings(bindings: Record<string, Keybinding>): void {
  try {
    const payload: PersistedKeybindings = { version: 1, bindings };
    localStorage.setItem(KEYBINDINGS_KEY, JSON.stringify(payload));
  } catch (e) {
    console.warn("saveKeybindings failed", e);
  }
}

export function loadPrefs(): StoredPrefs | null {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedPrefs;
    if (parsed.version !== 1 || !parsed.prefs) return null;
    return parsed.prefs;
  } catch (e) {
    console.warn("loadPrefs failed", e);
    return null;
  }
}

export function savePrefs(prefs: Prefs): void {
  try {
    const payload: PersistedPrefs = { version: 1, prefs };
    localStorage.setItem(PREFS_KEY, JSON.stringify(payload));
  } catch (e) {
    console.warn("savePrefs failed", e);
  }
}

export interface PaletteItem {
  id: string;
  label: string;
  command: string;
  autoRun: boolean;
}

interface PersistedPalette {
  version: 1;
  items: PaletteItem[];
}

export function loadPaletteItems(): PaletteItem[] | null {
  try {
    const raw = localStorage.getItem(PALETTE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedPalette;
    if (parsed.version !== 1 || !Array.isArray(parsed.items)) return null;
    return parsed.items;
  } catch (e) {
    console.warn("loadPaletteItems failed", e);
    return null;
  }
}

export function savePaletteItems(items: PaletteItem[]): void {
  try {
    const payload: PersistedPalette = { version: 1, items };
    localStorage.setItem(PALETTE_KEY, JSON.stringify(payload));
  } catch (e) {
    console.warn("savePaletteItems failed", e);
  }
}

/** A CLI tool that supports isolated multi-account login via an env var override. */
export type CliAgentKind = "claude" | "codex";

/**
 * How a profile's login was registered. `"oauth"` (the default) opens a
 * floating terminal to complete an interactive browser login; `"setup-token"`
 * (Claude only) links an already-issued `claude setup-token` value instead,
 * stored separately on disk — see `resolveProfileEnv` in
 * `useAccountProfiles.ts`. Optional so profiles saved before this field
 * existed are treated as `"oauth"`.
 */
export type AccountAuthMethod = "oauth" | "setup-token" | "environment";

/**
 * One saved login for a CLI agent: `configDir` is an isolated directory
 * (created on demand via the `resolve_account_dir` Tauri command) handed to
 * the agent as `CLAUDE_CONFIG_DIR`/`CODEX_HOME` so its login/session state
 * never mixes with another profile's. See `useAccountProfiles.ts`.
 */
export interface AccountProfile {
  id: string;
  agent: CliAgentKind;
  label: string;
  configDir: string;
  createdAt: number;
  authMethod?: AccountAuthMethod;
  env?: Record<string, string>;
}

interface PersistedAccounts {
  version: 1;
  items: AccountProfile[];
}

export function loadAccountProfiles(): AccountProfile[] | null {
  try {
    const raw = localStorage.getItem(ACCOUNTS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedAccounts;
    if (parsed.version !== 1 || !Array.isArray(parsed.items)) return null;
    return parsed.items;
  } catch (e) {
    console.warn("loadAccountProfiles failed", e);
    return null;
  }
}

export function saveAccountProfiles(items: AccountProfile[]): void {
  try {
    const payload: PersistedAccounts = { version: 1, items };
    localStorage.setItem(ACCOUNTS_KEY, JSON.stringify(payload));
  } catch (e) {
    console.warn("saveAccountProfiles failed", e);
  }
}
