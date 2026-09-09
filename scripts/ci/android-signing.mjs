import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const release = process.env.RELEASE_BUILD === "true";
const key = release ? process.env.ANDROID_KEYSTORE_BASE64 : "";
if (release) {
  const names = ["ANDROID_KEYSTORE_BASE64", "ANDROID_KEY_ALIAS", "ANDROID_KEY_PASSWORD", "ANDROID_STORE_PASSWORD"];
  if (names.some(name => process.env[name])) {
    for (const name of names) {
      if (!process.env[name]) throw new Error(`Missing Android signing secret: ${name}`);
    }
  }
}
if (key) {
  const keyPath = join(process.env.RUNNER_TEMP, "rhyme-upload.jks");
  writeFileSync(keyPath, Buffer.from(key, "base64"), { mode: 0o600 });
  appendFileSync(process.env.GITHUB_ENV, `ANDROID_SIGNING_STORE_FILE=${keyPath}\nANDROID_SIGNING_MODE=release\n`);
} else {
  appendFileSync(process.env.GITHUB_ENV, `ANDROID_SIGNING_MODE=${release ? "unsigned" : "debug"}\n`);
}

const gradle = "src-tauri/gen/android/app/build.gradle.kts";
if (!existsSync(gradle)) throw new Error("Android project has not been initialized");
const marker = "// rhyme-ci optional release signing";
if (!readFileSync(gradle, "utf8").includes(marker)) {
  // The generated project remains untouched in the checkout; only the runner
  // adds this environment-driven release signing block after initialization.
  appendFileSync(gradle, `
${marker}
android {
    val ciStore = System.getenv("ANDROID_SIGNING_STORE_FILE")
    if (!ciStore.isNullOrBlank()) {
        val ciSigning = signingConfigs.create("rhymeCiRelease") {
            storeFile = file(ciStore)
            storePassword = System.getenv("ANDROID_STORE_PASSWORD")
            keyAlias = System.getenv("ANDROID_KEY_ALIAS")
            keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
        }
        buildTypes.getByName("release") {
            signingConfig = ciSigning
        }
    }
}
`);
}
