import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (process.argv.length !== 3 || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version ?? "")) {
  console.error("Usage: pnpm version:set <major.minor.patch> (e.g. 0.0.2)");
  process.exit(1);
}

const root = new URL("../", import.meta.url);
const files = [
  ["package.json", /("version"\s*:\s*")[^"]+(".*)/],
  ["src-tauri/tauri.conf.json", /("version"\s*:\s*")[^"]+(".*)/],
  ["src-tauri/Cargo.toml", /(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m],
  ["src-tauri/Cargo.lock", /(\[\[package\]\]\r?\nname = "rhyme-terminal"\r?\nversion = ")[^"]+(")/],
];
// Read and validate every target before changing any files.
const updates = files.map(([path, pattern]) => {
  const url = new URL(path, root);
  const source = readFileSync(url, "utf8");
  if (!pattern.test(source)) throw new Error(`Application version missing in ${path}`);
  return [url, source, source.replace(pattern, (_match, before, after) => `${before}${version}${after}`)];
});
for (const [url, before, after] of updates) {
  if (before !== after) writeFileSync(url, after);
}
console.log(`Application version: ${version}`);
