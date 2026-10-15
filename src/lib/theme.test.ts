import { describe, expect, it } from "vitest";
import {
  ACCENT_THEMES,
  DEFAULT_ACCENT_THEME_ID,
  accentCssVars,
  accentThemeById,
  applyAccentTheme,
  contrastRatio,
  normalizeAccentThemeId,
  parseHex,
  readableOnAccent,
  withAlpha,
} from "./theme";

describe("normalizeAccentThemeId", () => {
  it("keeps a known id", () => {
    expect(normalizeAccentThemeId("aqua")).toBe("aqua");
  });

  it("falls back to the logo blue for unknown or missing values", () => {
    expect(DEFAULT_ACCENT_THEME_ID).toBe("rhyme");
    for (const value of [undefined, null, "", "nope", 7, {}]) {
      expect(normalizeAccentThemeId(value)).toBe("rhyme");
    }
  });
});

describe("accentThemeById", () => {
  it("resolves the matching theme", () => {
    expect(accentThemeById("amber").accent).toBe("#e2b341");
  });

  it("resolves the default for junk input", () => {
    expect(accentThemeById("junk").id).toBe(DEFAULT_ACCENT_THEME_ID);
  });
});

describe("theme catalogue", () => {
  it("has unique ids and 6-digit hex colors", () => {
    const ids = ACCENT_THEMES.map((theme) => theme.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const theme of ACCENT_THEMES) {
      expect(theme.accent).toMatch(/^#[0-9a-f]{6}$/);
      expect(theme.accentStrong).toMatch(/^#[0-9a-f]{6}$/);
    }
  });

  it("still offers the pre-theme teal so old installs can restore it", () => {
    expect(ACCENT_THEMES.find((theme) => theme.id === "aqua")?.accent).toBe("#4ec9b0");
  });

  it("keeps every accent legible on the #1e1e1e shell background", () => {
    for (const theme of ACCENT_THEMES) {
      expect(contrastRatio(theme.accent, "#1e1e1e")).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("keeps every accent legible under its own on-accent foreground", () => {
    for (const theme of ACCENT_THEMES) {
      expect(contrastRatio(theme.accent, readableOnAccent(theme.accent))).toBeGreaterThanOrEqual(4.5);
    }
  });
});

describe("parseHex", () => {
  it("expands shorthand", () => {
    expect(parseHex("#abc")).toEqual([0xaa, 0xbb, 0xcc]);
  });

  it("accepts a missing hash and mixed case", () => {
    expect(parseHex("4EC9B0")).toEqual([78, 201, 176]);
  });

  it("falls back to mid grey for unparseable input", () => {
    expect(parseHex("nonsense")).toEqual([128, 128, 128]);
  });
});

describe("withAlpha", () => {
  it("renders an rgba string", () => {
    expect(withAlpha("#4ec9b0", 0.12)).toBe("rgba(78, 201, 176, 0.12)");
  });
});

describe("readableOnAccent", () => {
  it("uses near-black on a light accent", () => {
    expect(readableOnAccent("#e2b341")).toBe("#151a20");
  });

  it("uses white on a dark accent", () => {
    expect(readableOnAccent("#123456")).toBe("#ffffff");
  });
});

describe("accentCssVars", () => {
  it("derives every accent variable from the base color", () => {
    expect(accentCssVars(accentThemeById("aqua"))).toEqual({
      "--accent": "#4ec9b0",
      "--accent-strong": "#6fd9c4",
      "--accent-soft": "rgba(78, 201, 176, 0.12)",
      "--accent-softer": "rgba(78, 201, 176, 0.22)",
      "--accent-border": "rgba(78, 201, 176, 0.32)",
      "--accent-on": "#151a20",
    });
  });
});

describe("applyAccentTheme", () => {
  it("writes the variables onto the given element", () => {
    const set = new Map<string, string>();
    const root = { style: { setProperty: (name: string, value: string) => set.set(name, value) } };
    applyAccentTheme(accentThemeById("rhyme"), root as unknown as HTMLElement);
    expect(set.get("--accent")).toBe("#5a9bff");
    expect(set.get("--accent-on")).toBe("#151a20");
    expect(set.size).toBe(6);
  });
});
