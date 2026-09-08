import { describe, expect, it, vi } from "vitest";
import { envError, sessionAccountEnv } from "./account-env";
import { envForProfile, resolveProfileEnv } from "../composables/useAccountProfiles";
import type { AccountProfile } from "./persistence";
vi.mock("./tauri", () => ({ api: { getAccountToken: vi.fn().mockResolvedValue("linked-token") } }));
const profile: AccountProfile = { id: "test", agent: "claude", label: "Test", configDir: "C:\\profiles\\test", createdAt: 0 };
describe("account environment", () => {
  it("rejects invalid names, case-insensitive duplicates and reserved settings", () => {
    expect(envError([{ name: "HTTP_PROXY", value: "a" }, { name: "http_proxy", value: "b" }])).not.toBeNull();
    expect(envError([{ name: "BAD=NAME", value: "" }])).not.toBeNull();
    expect(envError([{ name: "codex_home", value: "" }], ["CODEX_HOME"])).not.toBeNull();
    expect(envError([{ name: "VALID", value: "\0" }])).not.toBeNull();
    expect(envError([{ name: "MY_KEY", value: "" }])).toBeNull();
  });
  it("preserves custom values and the isolated login directory", () => {
    expect(envForProfile({ ...profile, env: { HTTP_PROXY: "http://localhost:8080", CLAUDE_CONFIG_DIR: "wrong" } })).toEqual({ HTTP_PROXY: "http://localhost:8080", CLAUDE_CONFIG_DIR: profile.configDir });
    expect(envForProfile({ ...profile, agent: "codex", env: { OPENAI_BASE_URL: "http://localhost/v1" } })).toEqual({ OPENAI_BASE_URL: "http://localhost/v1", CODEX_HOME: profile.configDir });
  });
  it("keeps linked token precedence", async () => {
    expect(await resolveProfileEnv({ ...profile, authMethod: "setup-token", env: { CLAUDE_CODE_OAUTH_TOKEN: "wrong", HTTP_PROXY: "proxy" } })).toEqual({ CLAUDE_CONFIG_DIR: profile.configDir, CLAUDE_CODE_OAUTH_TOKEN: "linked-token", HTTP_PROXY: "proxy" });
  });
  it("uses only the launched system agent's settings and does not leak them into profiles or plain shells", () => {
    const system = { claude: { HTTP_PROXY: "claude" }, codex: { HTTP_PROXY: "codex" } };
    expect(sessionAccountEnv("claude", undefined, system)).toEqual(system.claude);
    expect(sessionAccountEnv("codex", undefined, system)).toEqual(system.codex);
    expect(sessionAccountEnv("codex", { CODEX_HOME: "isolated" }, system)).toEqual({ CODEX_HOME: "isolated" });
    expect(sessionAccountEnv(undefined, undefined, system)).toBeUndefined();
    expect(sessionAccountEnv("claude", { HTTP_PROXY: "" }, system)).toEqual({ HTTP_PROXY: "" });
  });
});
