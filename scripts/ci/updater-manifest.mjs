// Writes the `latest.json` the in-app updater polls. It lists the signed
// Windows NSIS installer and macOS app archive found under ARTIFACTS_DIR,
// pointing each at its asset on the GitHub release being published.
//
// Exits 0 without writing anything when no updater signatures exist (the
// signing secret was not configured), so releases keep working without it.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

const dir = process.env.ARTIFACTS_DIR ?? "artifacts";
const tag = process.env.RELEASE_TAG;
const repo = process.env.GH_REPO;
const output = process.env.MANIFEST_PATH ?? join(dir, "latest.json");
if (!tag || !repo) throw new Error("RELEASE_TAG and GH_REPO are required");

function files(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap(entry =>
    entry.isDirectory() ? files(join(root, entry.name)) : [join(root, entry.name)]);
}
const all = files(dir);
const signatures = all.filter(path => path.endsWith(".sig"));
if (signatures.length === 0) {
  console.log("No updater signatures found; skipping latest.json");
  process.exit(0);
}

function entry(suffix, label) {
  const matches = signatures.filter(path => path.endsWith(`${suffix}.sig`));
  if (matches.length !== 1) throw new Error(`Expected one ${label} updater signature, found ${matches.length}`);
  const asset = basename(matches[0]).slice(0, -".sig".length);
  if (!all.some(path => basename(path) === asset)) throw new Error(`Missing updater package ${asset}`);
  return {
    signature: readFileSync(matches[0], "utf8").trim(),
    url: `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(asset)}`,
  };
}

const windows = entry("-setup.exe", "Windows NSIS");
// The macOS release job runs on macos-latest, which is Apple silicon.
const macos = entry(".app.tar.gz", "macOS app");
const manifest = {
  version: tag.replace(/^v/, ""),
  notes: process.env.RELEASE_NOTES ?? "",
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": windows,
    "windows-x86_64-nsis": windows,
    "darwin-aarch64": macos,
    "darwin-aarch64-app": macos,
  },
};
writeFileSync(output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Wrote ${output} for ${tag}`);
