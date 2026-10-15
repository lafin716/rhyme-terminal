import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, readdir, readFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';
const binary = resolve('src-tauri/target/debug/winmuxd.exe');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function run(cancel, args = ['resume', '11111111-1111-4111-8111-111111111111', 'space argument', '$(must-stay-literal)', '`literal`']) {
  const root = await mkdtemp(join(tmpdir(), 'rhyme-bridge-qa-'));
  await writeFile(join(root, 'heartbeat'), '1');
  const marker = join(root, 'child.json');
  const child = spawn(binary, ['--rhyme-loop-agent', 'codex', ...args], {windowsHide:true,env:{...process.env,RHYME_LOOP_DIR:root},stdio:['ignore','pipe','pipe']});
  let stderr = '';
  let stdout = '';
  child.stdout.on('data',b=>{stdout+=b;});
  child.stderr.on('data', b => { stderr += b; });
  const done = new Promise((resolve, reject) => { child.on('exit', code => resolve(code)); child.on('error', reject); });
  try {
    let name;
    for (let i = 0; i < 300; i++) {
      name = (await readdir(root)).find(n => n.startsWith('request-') && n.endsWith('.json'));
      if (name) break;
      await sleep(100);
    }
    assert(name, `missing request: ${root}, exit=${child.exitCode}, stdout=${stdout}, stderr=${stderr}`);
    const request = JSON.parse(await readFile(join(root, name), 'utf8'));
    assert.deepEqual(request.args, args);
    assert.equal(request.provider, 'codex');
    await sleep(400);
    await assert.rejects(access(marker));
    const launch = cancel ? { cancel: true } : { shell: process.execPath, args: ['-e', 'require("fs").writeFileSync(process.argv[1],JSON.stringify(process.argv.slice(2)));console.log("NATIVE_OUTPUT_MARKER")', marker, ...request.args], env: {}, managed: true };
    await writeFile(join(root, `response-${request.id}.json`), JSON.stringify(launch));
    assert.equal(await done, 0, stderr);
    if (cancel) await assert.rejects(access(marker));
    else {assert.deepEqual(JSON.parse(await readFile(marker, 'utf8')), args);assert(stdout.includes('NATIVE_OUTPUT_MARKER'));}
    return { cancel, passed: true, root };
  } finally { if (child.exitCode === null) child.kill(); }
}
const result = [await run(false), await run(true), await run(false, [])];
await writeFile('.scratch/agent-loop-runtime-qa/bridge-report.json', JSON.stringify(result, null, 2));
console.log(JSON.stringify(result));
