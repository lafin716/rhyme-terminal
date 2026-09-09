import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, writeFileSync, rmSync } from "node:fs";
import { resolve, join } from "node:path";
import { randomUUID } from "node:crypto";

const binary = resolve("src-tauri/target/release/bundle/macos/rhyme-terminal.app/Contents/MacOS/rhyme-terminal");
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
function launch(t, root, mode, payload) {
  const child = spawn(binary, [mode, root], {
    env: { ...process.env, RHYME_LOOP_ATTEMPT_DIR: root },
    stdio: ["pipe", "pipe", "pipe"],
  });
  t.after(() => { if (child.exitCode === null) child.kill("SIGKILL"); });
  let output = "", error = "";
  child.stdout.on("data", bytes => { output += bytes; });
  child.stderr.on("data", bytes => { error += bytes; });
  const result = new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("close", code => resolve({ code, output, error }));
  });
  child.stdin.end(JSON.stringify(payload));
  return { child, result };
}
function fixture(t) {
  const root = mkdtempSync("/tmp/rhyme-smoke-");
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "events"));
  return root;
}
async function waitForEvent(root) {
  for (let i = 0; i < 100; i++) {
    const files = readdirSync(join(root, "events")).filter(name => name.endsWith(".json"));
    if (files.length) return JSON.parse(readFileSync(join(root, "events", files[0]), "utf8"));
    await pause(50);
  }
  throw new Error("Lifecycle event was not delivered");
}
test("packaged Unix helper records lifecycle metadata without raw input", { timeout: 10000 }, async t => {
  const root = fixture(t);
  const session = randomUUID();
  const { result } = launch(t, root, "--winmux-hook", {
    hook_event_name: "PostToolUse", session_id: session, tool_name: "Bash",
    tool_input: { command: "private-prompt", run_in_background: true },
    tool_response: "private-output",
  });
  assert.deepEqual(await result, { code: 0, output: "{}\n", error: "" });
  const event = await waitForEvent(root);
  assert.equal(event.sessionId, session);
  assert.equal(event.unknownBackground, true);
  assert.ok(!JSON.stringify(event).includes("private-"));
});
test("startup waits for matching approval before acknowledging readiness", { timeout: 10000 }, async t => {
  const root = fixture(t), session = randomUUID();
  const { child, result } = launch(t, root, "--winmux-hook", { hook_event_name: "SessionStart", session_id: session });
  await waitForEvent(root);
  writeFileSync(join(root, "start-approved.json"), JSON.stringify({ sessionId: randomUUID() }));
  await pause(250);
  assert.equal(child.exitCode, null);
  writeFileSync(join(root, "start-approved.json"), JSON.stringify({ sessionId: session }));
  assert.equal((await result).code, 0);
  assert.equal(JSON.parse(readFileSync(join(root, "startup-ready.json"), "utf8")).sessionId, session);
});
test("switch boundary stays held until the controller explicitly resumes", { timeout: 10000 }, async t => {
  const root = fixture(t);
  writeFileSync(join(root, "control.json"), JSON.stringify({ switchRequested: true }));
  const { child, result } = launch(t, root, "--winmux-hook", { hook_event_name: "PreToolUse", session_id: randomUUID() });
  assert.equal((await waitForEvent(root)).boundary, true);
  await pause(250);
  assert.equal(child.exitCode, null);
  writeFileSync(join(root, "control.json"), JSON.stringify({ switchRequested: false }));
  assert.equal((await result).output, "{}\n");
});
test("statusline keeps original output and stores quota fields only", { timeout: 10000 }, async t => {
  const root = fixture(t);
  writeFileSync(join(root, "winmux-statusline.json"), JSON.stringify({ original: { command: "cat >/dev/null; printf original" } }));
  const { result } = launch(t, root, "--winmux-statusline", { model: { name: "private-model" }, rate_limits: { five_hour: { used_percentage: 12, resets_at: 123 } } });
  assert.equal((await result).output, "original");
  const stored = JSON.parse(readFileSync(join(root, "winmux-usage.json"), "utf8"));
  assert.equal(stored.rate_limits.five_hour.used_percentage, 12);
  assert.ok(!JSON.stringify(stored).includes("private-model"));
});
