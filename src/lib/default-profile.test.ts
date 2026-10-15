import { describe, expect, it } from "vitest";
import { resolveDefaultProfile } from "./default-profile";
import type { AccountProfile } from "../composables/useAccountProfiles";

function makeProfile(overrides: Partial<AccountProfile> = {}): AccountProfile {
  return {
    id: "acct_1",
    agent: "claude",
    label: "Personal",
    configDir: "C:\\Users\\user\\AppData\\Local\\winmux\\accounts\\claude\\acct_1",
    createdAt: 0,
    authMethod: "oauth",
    ...overrides,
  };
}

describe("resolveDefaultProfile", () => {
  it("returns null (System) when defaultProfileId[agent] is null", () => {
    const profiles = [makeProfile()];
    const result = resolveDefaultProfile("claude", profiles, { claude: null, codex: null });
    expect(result).toBeNull();
  });

  it("resolves the matching profile when the id is set and present", () => {
    const profile = makeProfile();
    const result = resolveDefaultProfile("claude", [profile], {
      claude: profile.id,
      codex: null,
    });
    expect(result).toBe(profile);
  });

  it("falls back to System when the stored id no longer matches any profile", () => {
    // e.g. the profile was removed since being chosen as the default.
    const profiles = [makeProfile()];
    const result = resolveDefaultProfile("claude", profiles, {
      claude: "acct_removed",
      codex: null,
    });
    expect(result).toBeNull();
  });

  it("falls back to System when the stored id belongs to a profile for the other agent", () => {
    const codexProfile = makeProfile({ id: "acct_codex", agent: "codex" });
    const result = resolveDefaultProfile("claude", [codexProfile], {
      claude: "acct_codex",
      codex: null,
    });
    expect(result).toBeNull();
  });
});
