import { readFileSync } from "node:fs";
const tag = process.env.RELEASE_TAG;
const pkg = JSON.parse(readFileSync("package.json", "utf8")).version;
const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version;
const cargo = readFileSync("src-tauri/Cargo.toml", "utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (tag !== `v${pkg}` || pkg !== config || pkg !== cargo) {
  throw new Error(`Version mismatch: tag=${tag}, package=${pkg}, Tauri=${config}, Cargo=${cargo}`);
}
console.log(`Release version verified: ${tag}`);
