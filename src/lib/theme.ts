/**
 * Accent themes: the single "main color" the whole shell is tinted with.
 *
 * Every component reads the accent through the CSS custom properties written
 * by {@link accentCssVars} onto the document root, so switching a theme is one
 * variable write rather than a re-render. The derived alpha/foreground values
 * are computed here (instead of being hand-written per theme) so a new theme
 * only has to supply its base color.
 */

export type AccentThemeId =
  | "rhyme"
  | "violet"
  | "aqua"
  | "amber"
  | "ember"
  | "rose";

export interface AccentTheme {
  id: AccentThemeId;
  /** English label — a stable `t()` key, like every other user-facing string. */
  label: string;
  /** Base accent, used as-is for text, borders and fills. */
  accent: string;
  /** Slightly lifted accent for hover/pressed affordances. */
  accentStrong: string;
}

/**
 * The logo's blue is the default: `rhyme` is sampled from the middle of the
 * mark's purple→blue→cyan gradient, and `violet`/`aqua` sit at either end of
 * it. `aqua` is the accent the app shipped with before themes existed, so
 * pre-theme installs can get their old look back by picking it.
 */
export const ACCENT_THEMES: readonly AccentTheme[] = [
  {
    id: "rhyme",
    label: "Rhyme Blue",
    accent: "#5a9bff",
    accentStrong: "#82b4ff",
  },
  {
    id: "violet",
    label: "Rhyme Violet",
    accent: "#9a86ff",
    accentStrong: "#b3a3ff",
  },
  {
    id: "aqua",
    label: "Aqua Teal",
    accent: "#4ec9b0",
    accentStrong: "#6fd9c4",
  },
  {
    id: "amber",
    label: "Amber",
    accent: "#e2b341",
    accentStrong: "#efc862",
  },
  {
    id: "ember",
    label: "Ember",
    accent: "#e08a63",
    accentStrong: "#eda684",
  },
  {
    id: "rose",
    label: "Rose",
    accent: "#f27ab0",
    accentStrong: "#f79ac5",
  },
];

export const DEFAULT_ACCENT_THEME_ID: AccentThemeId = "rhyme";

export function normalizeAccentThemeId(value: unknown): AccentThemeId {
  return ACCENT_THEMES.some((theme) => theme.id === value)
    ? (value as AccentThemeId)
    : DEFAULT_ACCENT_THEME_ID;
}

export function accentThemeById(value: unknown): AccentTheme {
  const id = normalizeAccentThemeId(value);
  // `normalizeAccentThemeId` only ever returns an id that is in the list.
  return ACCENT_THEMES.find((theme) => theme.id === id)!;
}

/** `#rgb`/`#rrggbb` → `[r, g, b]` in 0–255. Unparseable input falls back to mid grey. */
export function parseHex(hex: string): [number, number, number] {
  const body = hex.trim().replace(/^#/, "");
  const full = body.length === 3
    ? body.split("").map((ch) => ch + ch).join("")
    : body;
  if (!/^[0-9a-fA-F]{6}$/.test(full)) return [128, 128, 128];
  return [
    parseInt(full.slice(0, 2), 16),
    parseInt(full.slice(2, 4), 16),
    parseInt(full.slice(4, 6), 16),
  ];
}

export function withAlpha(hex: string, alpha: number): string {
  const [r, g, b] = parseHex(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** WCAG relative luminance, 0 (black) – 1 (white). */
export function relativeLuminance(hex: string): number {
  const [r, g, b] = parseHex(hex).map((channel) => {
    const c = channel / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrastRatio(a: string, b: string): number {
  const [light, dark] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

const ON_ACCENT_DARK = "#151a20";
const ON_ACCENT_LIGHT = "#ffffff";

/**
 * Foreground for text sitting *on* an accent fill (the status bar, badges).
 * Picks whichever of near-black/white reads better rather than assuming the
 * accent is always light, so a future dark accent still stays legible.
 */
export function readableOnAccent(accent: string): string {
  return contrastRatio(accent, ON_ACCENT_DARK) >= contrastRatio(accent, ON_ACCENT_LIGHT)
    ? ON_ACCENT_DARK
    : ON_ACCENT_LIGHT;
}

/** The custom properties every component's accent styling resolves through. */
export function accentCssVars(theme: AccentTheme): Record<string, string> {
  return {
    "--accent": theme.accent,
    "--accent-strong": theme.accentStrong,
    "--accent-soft": withAlpha(theme.accent, 0.12),
    "--accent-softer": withAlpha(theme.accent, 0.22),
    "--accent-border": withAlpha(theme.accent, 0.32),
    "--accent-on": readableOnAccent(theme.accent),
  };
}

export function applyAccentTheme(theme: AccentTheme, root: HTMLElement): void {
  for (const [name, value] of Object.entries(accentCssVars(theme))) {
    root.style.setProperty(name, value);
  }
}
