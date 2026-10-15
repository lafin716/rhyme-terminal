import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { computed } from "vue";
import { DEFAULT_LOCALE, normalizeLocale, translate } from "./i18n";
import { ko } from "./locales/ko";
import { ACTIONS } from "./keybindings";
import { formatUsageReset } from "./usage-status";

describe("translations", () => {
  it("defaults missing and unsupported languages to Korean", () => {
    expect(DEFAULT_LOCALE).toBe("ko");
    for (const value of [undefined, null, "ja", {}, 1]) expect(normalizeLocale(value)).toBe("ko");
    expect(normalizeLocale("en")).toBe("en");
  });

  it("translates messages and preserves interpolated user content literally", () => {
    expect(translate("ko", "Settings")).toBe("설정");
    expect(translate("en", "Settings")).toBe("Settings");
    expect(translate("ko", 'Kill session "{name}"?', { name: "{count} $& <test>" }))
      .toBe('세션 "{count} $& <test>"을 종료할까요?');
    expect(translate("en", "Kill {count} terminal sessions?", { count: 0 }))
      .toBe("Kill 0 terminal sessions?");
    expect(translate("ko", "Untranslated message")).toBe("Untranslated message");
  });

  it("has intact Korean text and matching interpolation variables for every message", () => {
    const variables = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort();
    for (const [key, value] of Object.entries(ko)) {
      expect(value, key).toMatch(/[가-힣]/);
      expect(value, key).not.toContain("\uFFFD");
      expect(variables(value), key).toEqual(variables(key));
    }
    for (const action of ACTIONS) expect(ko[action.label], action.id).toBeTruthy();
  });

  it("localizes status text in both languages", () => {
    expect(formatUsageReset({ percentUsed: null, resetsAt: null }, 0, "ko")).toBe("연동 필요");
    expect(formatUsageReset({ percentUsed: null, resetsAt: null }, 0, "en")).toBe("Not connected");
    expect(formatUsageReset({ percentUsed: 50, resetsAt: 90 * 60_000 }, 0, "en")).toBe("Resets in 1h 30m");
  });
});

describe("language preferences", () => {
  let storage: Map<string, string>;
  beforeEach(() => {
    vi.resetModules();
    storage = new Map();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("uses Korean for existing preferences without changing other settings", async () => {
    storage.set("winmux:prefs:v1", JSON.stringify({ version: 1, prefs: { skipKillSessionConfirm: true } }));
    const { usePrefs, loadPrefsFromStorage } = await import("../composables/usePrefs");
    loadPrefsFromStorage();
    expect(usePrefs().prefs.language).toBe("ko");
    expect(usePrefs().prefs.skipKillSessionConfirm).toBe(true);
  });

  it("updates reactive labels immediately and restores the saved choice", async () => {
    const { usePrefs, setPref } = await import("../composables/usePrefs");
    const { t } = await import("../composables/useI18n");
    const label = computed(() => t("Settings"));
    expect(label.value).toBe("설정");
    setPref("language", "en");
    expect(label.value).toBe("Settings");
    expect(usePrefs().prefs.language).toBe("en");
    vi.resetModules();
    const reloaded = await import("../composables/usePrefs");
    reloaded.loadPrefsFromStorage();
    expect(reloaded.usePrefs().prefs.language).toBe("en");
    reloaded.setPref("language", "ko");
    expect(JSON.parse(storage.get("winmux:prefs:v1")!).prefs.language).toBe("ko");
  });
});
