import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const scripts = fileURLToPath(new URL(".", import.meta.url));
function fixture(fn) {
  const parent = resolve(tmpdir());
  const root = mkdtempSync(join(parent, "rhyme-ci-test-"));
  try { fn(root); } finally {
    assert.ok(resolve(root).startsWith(parent + sep));
    assert.ok(basename(root).startsWith("rhyme-ci-test-"));
    rmSync(root, { recursive: true, force: true });
  }
}
function write(root, path, text) {
  const destination = join(root, path);
  mkdirSync(resolve(destination, ".."), { recursive: true });
  writeFileSync(destination, text);
}
function run(root, script, env = {}) {
  return spawnSync(process.execPath, [join(scripts, script)], {
    cwd: root,
    encoding: "utf8",
    env: {
      ...process.env,
      ANDROID_KEYSTORE_BASE64: "",
      ANDROID_KEY_ALIAS: "",
      ANDROID_KEY_PASSWORD: "",
      ANDROID_STORE_PASSWORD: "",
      RUNNER_TEMP: root,
      GITHUB_ENV: join(root, "github-env"),
      ...env,
    },
  });
}

test("release requires the tag and all application versions to match", () => fixture(root => {
  write(root, "package.json", '{"version":"0.1.0"}');
  write(root, "src-tauri/tauri.conf.json", '{"version":"0.1.0"}');
  write(root, "src-tauri/Cargo.toml", '[package]\nversion = "0.1.0"\n');
  assert.equal(run(root, "check-release-version.mjs", { RELEASE_TAG: "v0.1.0" }).status, 0);
  assert.notEqual(run(root, "check-release-version.mjs", { RELEASE_TAG: "v0.2.0" }).status, 0);
  write(root, "src-tauri/Cargo.toml", '[package]\nversion = "0.2.0"\n');
  assert.notEqual(run(root, "check-release-version.mjs", { RELEASE_TAG: "v0.1.0" }).status, 0);
}));

test("Android packages use actual variant paths and explicit signing labels", () => fixture(root => {
  write(root, "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk", "apk");
  write(root, "src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab", "aab");
  const result = run(root, "collect-android.mjs", { RELEASE_BUILD: "true", ANDROID_SIGNING_MODE: "unsigned" });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(root, "artifacts/android/rhyme-terminal-android-arm64-unsigned.apk"), "utf8"), "apk");
  assert.equal(readFileSync(join(root, "artifacts/android/rhyme-terminal-android-arm64-unsigned.aab"), "utf8"), "aab");
}));

test("Android collection rejects missing or ambiguous outputs", () => fixture(root => {
  write(root, "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-debug.apk", "apk");
  assert.notEqual(run(root, "collect-android.mjs").status, 0);
  write(root, "src-tauri/gen/android/app/build/outputs/bundle/universalDebug/app-debug.aab", "aab");
  assert.equal(run(root, "collect-android.mjs").status, 0);
  write(root, "src-tauri/gen/android/app/build/outputs/apk/arm64/debug/extra.apk", "apk");
  assert.notEqual(run(root, "collect-android.mjs").status, 0);
}));

test("Android release without secrets stays unsigned and Gradle patch is idempotent", () => fixture(root => {
  const gradle = "src-tauri/gen/android/app/build.gradle.kts";
  write(root, gradle, "android {}\n");
  const first = run(root, "android-signing.mjs", { RELEASE_BUILD: "true" });
  assert.equal(first.status, 0, first.stderr);
  assert.match(readFileSync(join(root, "github-env"), "utf8"), /ANDROID_SIGNING_MODE=unsigned/);
  const patched = readFileSync(join(root, gradle), "utf8");
  assert.equal(run(root, "android-signing.mjs", { RELEASE_BUILD: "true" }).status, 0);
  assert.equal(readFileSync(join(root, gradle), "utf8"), patched);
}));

test("Android signing rejects incomplete secrets and never writes passwords into Gradle", () => fixture(root => {
  const gradle = "src-tauri/gen/android/app/build.gradle.kts";
  write(root, gradle, "android {}\n");
  const env = { RELEASE_BUILD: "true", ANDROID_KEYSTORE_BASE64: Buffer.from("test-fixture-only").toString("base64") };
  assert.notEqual(run(root, "android-signing.mjs", env).status, 0);
  const result = run(root, "android-signing.mjs", {
    ...env, ANDROID_KEY_ALIAS: "fixture-alias",
    ANDROID_KEY_PASSWORD: "fixture-key-password", ANDROID_STORE_PASSWORD: "fixture-store-password",
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(root, "rhyme-upload.jks"), "utf8"), "test-fixture-only");
  assert.doesNotMatch(readFileSync(join(root, gradle), "utf8"), /fixture-/);
  assert.doesNotMatch(result.stdout + result.stderr, /fixture-/);
}));
