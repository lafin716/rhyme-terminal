import assert from "node:assert/strict";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
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

test("Android packages use actual variant paths and release signing labels", () => fixture(root => {
  write(root, "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk", "apk");
  write(root, "src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab", "aab");
  const result = run(root, "collect-android.mjs", { RELEASE_BUILD: "true", ANDROID_SIGNING_MODE: "release" });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(root, "artifacts/android/rhyme-terminal-android-arm64-release.apk"), "utf8"), "apk");
  assert.equal(readFileSync(join(root, "artifacts/android/rhyme-terminal-android-arm64-release.aab"), "utf8"), "aab");
}));

test("Android release collection rejects unsigned, debug, and missing signing mode", () => fixture(root => {
  for (const mode of ["unsigned", "debug", ""]) {
    const result = run(root, "collect-android.mjs", { RELEASE_BUILD: "true", ANDROID_SIGNING_MODE: mode });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /require release signing/);
    assert.equal(existsSync(join(root, "artifacts/android")), false);
  }
}));

test("Android collection rejects missing or ambiguous outputs", () => fixture(root => {
  write(root, "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-debug.apk", "apk");
  assert.notEqual(run(root, "collect-android.mjs").status, 0);
  write(root, "src-tauri/gen/android/app/build/outputs/bundle/universalDebug/app-debug.aab", "aab");
  assert.equal(run(root, "collect-android.mjs").status, 0);
  write(root, "src-tauri/gen/android/app/build/outputs/apk/arm64/debug/extra.apk", "apk");
  assert.notEqual(run(root, "collect-android.mjs").status, 0);
}));

test("Android release without secrets fails before writing signing material or Gradle", () => fixture(root => {
  const gradle = "src-tauri/gen/android/app/build.gradle.kts";
  write(root, gradle, "android {}\n");
  const result = run(root, "android-signing.mjs", { RELEASE_BUILD: "true" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Missing Android signing secret: ANDROID_KEYSTORE_BASE64/);
  assert.equal(readFileSync(join(root, gradle), "utf8"), "android {}\n");
  assert.equal(existsSync(join(root, "github-env")), false);
  assert.equal(existsSync(join(root, "rhyme-upload.jks")), false);
}));

test("Android debug ignores signing secrets and Gradle patch is idempotent", () => fixture(root => {
  const gradle = "src-tauri/gen/android/app/build.gradle.kts";
  write(root, gradle, "android {}\n");
  const first = run(root, "android-signing.mjs", { RELEASE_BUILD: "false", ANDROID_KEYSTORE_BASE64: "ignored" });
  assert.equal(first.status, 0, first.stderr);
  assert.match(readFileSync(join(root, "github-env"), "utf8"), /ANDROID_SIGNING_MODE=debug/);
  assert.equal(existsSync(join(root, "rhyme-upload.jks")), false);
  const patched = readFileSync(join(root, gradle), "utf8");
  assert.equal(run(root, "android-signing.mjs", { RELEASE_BUILD: "false" }).status, 0);
  assert.equal(readFileSync(join(root, gradle), "utf8"), patched);
}));

test("Android signing rejects incomplete secrets and never writes passwords into Gradle", () => fixture(root => {
  const gradle = "src-tauri/gen/android/app/build.gradle.kts";
  write(root, gradle, "android {}\n");
  const env = {
    RELEASE_BUILD: "true", ANDROID_KEYSTORE_BASE64: Buffer.from("test-fixture-only").toString("base64"),
    ANDROID_KEY_ALIAS: "fixture-alias",
    ANDROID_KEY_PASSWORD: "fixture-key-password", ANDROID_STORE_PASSWORD: "fixture-store-password",
  };
  for (const name of ["ANDROID_KEYSTORE_BASE64", "ANDROID_KEY_ALIAS", "ANDROID_KEY_PASSWORD", "ANDROID_STORE_PASSWORD"]) {
    const rejected = run(root, "android-signing.mjs", { ...env, [name]: "" });
    assert.notEqual(rejected.status, 0);
    assert.ok(rejected.stderr.includes(`Missing Android signing secret: ${name}`));
    assert.equal(existsSync(join(root, "rhyme-upload.jks")), false);
    assert.equal(readFileSync(join(root, gradle), "utf8"), "android {}\n");
  }
  const result = run(root, "android-signing.mjs", env);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(root, "rhyme-upload.jks"), "utf8"), "test-fixture-only");
  assert.equal(statSync(join(root, "rhyme-upload.jks")).mode & 0o777, 0o600);
  assert.match(readFileSync(join(root, "github-env"), "utf8"), /ANDROID_SIGNING_MODE=release/);
  const patched = readFileSync(join(root, gradle), "utf8");
  assert.match(patched, /signingConfig = ciSigning/);
  assert.equal(run(root, "android-signing.mjs", env).status, 0);
  assert.equal(readFileSync(join(root, gradle), "utf8"), patched);
  assert.doesNotMatch(readFileSync(join(root, gradle), "utf8"), /fixture-/);
  assert.doesNotMatch(result.stdout + result.stderr, /fixture-/);
}));
