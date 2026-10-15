import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import { join } from "node:path";

function files(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry =>
    entry.isDirectory() ? files(join(directory, entry.name)) : [join(directory, entry.name)]);
}
const variant = process.env.RELEASE_BUILD === "true" ? "release" : "debug";
const mode = process.env.ANDROID_SIGNING_MODE || "debug";
const outputs = files("src-tauri/gen/android/app/build/outputs");
mkdirSync("artifacts/android", { recursive: true });
for (const extension of ["apk", "aab"]) {
  const matches = outputs.filter(path => path.endsWith(`.${extension}`) &&
    path.split(/[\\/]/).some(part => part.toLowerCase().endsWith(variant)));
  if (matches.length !== 1) {
    throw new Error(`Expected one ${variant} ${extension}, found ${matches.length}: ${matches.join(", ")}`);
  }
  copyFileSync(matches[0], `artifacts/android/rhyme-terminal-android-arm64-${mode}.${extension}`);
}
