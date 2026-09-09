// Capture the real frontend with deterministic demo data, never live accounts or PTYs.
// Run after pnpm.cmd build. Requires Chrome (or CHROME_PATH) and Node.js 22+.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';

import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const root = path.join(repo, 'dist');
const output = path.join(repo, 'docs/images');
await fs.access(path.join(root, 'index.html'));
await fs.mkdir(output, { recursive: true });
const captureWork = path.join(repo, '.scratch/readme-capture');
await fs.mkdir(captureWork, { recursive: true });
const profile = await fs.mkdtemp(path.join(captureWork, 'chrome-'));
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const server = http.createServer(async (req, res) => {
  const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
  const file = path.resolve(root, '.' + (pathname === '/' ? '/index.html' : pathname));
  if (!file.startsWith(root + path.sep)) { res.writeHead(403).end(); return; }
  try {
    const data = await fs.readFile(file);
    const mime = { '.js': 'application/javascript', '.css': 'text/css', '.html': 'text/html', '.png': 'image/png', '.svg': 'image/svg+xml', '.ttf': 'font/ttf' };
    res.setHeader('Content-Type', mime[path.extname(file)] || 'application/octet-stream');
    res.end(data);
  } catch { res.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const chrome = spawn(process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', [
  '--headless=new', '--disable-gpu', '--no-first-run',
  '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank',
], { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
let launchError;
chrome.stderr.on('data', data => process.stderr.write(data));
chrome.on('exit', code => { launchError = new Error('Chrome exited: ' + code); });
chrome.on('error', error => { launchError = error; });
let socket;
try {
  let port;
  for (let i = 0; i < 300; i++) {
    if (launchError) throw launchError;
    try { port = Number((await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]); break; }
    catch { await delay(200); }
  }
  assert(port, 'Chrome did not start');
  const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  socket = new WebSocket(tabs.find(tab => tab.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let sequence = 0;
  const pending = new Map();
  const errors = [];
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    if (message.id) {
      const task = pending.get(message.id);
      pending.delete(message.id);
      clearTimeout(task?.timer);
      message.error ? task?.reject(message.error) : task?.resolve(message.result);
    }
    if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails);
  };
  const cdp = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`${method} timed out`)); }, 15000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const result = await cdp('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result?.value;
  };
  const capture = async name => {
    await evaluate('document.fonts.ready');
    await delay(350);
    const result = await cdp('Page.captureScreenshot', { format: 'png' });
    await fs.writeFile(path.join(output, name + '.png'), Buffer.from(result.data, 'base64'));
    console.log(`Captured docs/images/${name}.png`);
  };
  await cdp('Page.enable');
  await cdp('Runtime.enable');
  await cdp('Emulation.setDeviceMetricsOverride', { width: 1600, height: 960, deviceScaleFactor: 1, mobile: false });
  await cdp('Page.addScriptToEvaluateOnNewDocument', { source: `(${seedDemo.toString()})()` });
  await cdp('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
  for (let i = 0; i < 100; i++) {
    if (await evaluate("document.querySelectorAll('.xterm-screen').length===3")) break;
    await delay(200);
  }
  assert.equal(await evaluate("document.querySelectorAll('.xterm-screen').length"), 3);
  await delay(1000);
  await capture('terminal-workspace');

  await evaluate("document.querySelector('.open-flow').click()");
  await delay(500);
  await evaluate(`(() => { const select = document.querySelector('[aria-label="저장된 Flow 버전"]'); select.value = '0'; select.dispatchEvent(new Event('change', { bubbles: true })); })()`);
  assert.equal(await evaluate("document.querySelectorAll('.flow-page .node').length"), 5);
  await capture('rhyme-flow');
  await evaluate("document.querySelector('.flow-page .back-to-app').click()");
  await evaluate("document.querySelector('.settings-button').click()");
  await delay(200);
  await evaluate("[...document.querySelectorAll('.settings-shell .nav-item')].find(e => e.textContent.includes('AI 제공자')).click()");
  await delay(200);
  assert.equal(await evaluate("document.querySelector('.panel-title').textContent.trim()"), 'AI 제공자');
  assert(await evaluate("!!document.querySelector('.app-icon img') && !document.querySelector('.shell-menu .corner-toggle') && !document.querySelector('.window-controls .panel-toggle')"));
  await capture('ai-providers');
  assert.equal(errors.length, 0, JSON.stringify(errors));
  console.log('Screenshots checked: 3 panes, 5 Flow nodes, AI provider heading and page controls.');
} finally {
  socket?.close();
  chrome.kill();
  server.close();
  // The isolated profile is retained in .scratch/readme-capture for troubleshooting.
}

function seedDemo() {
  const cwd = 'C:\\projects\\rhyme-terminal';
  const shell = { preset: 'windows-powershell', program: 'powershell.exe', args: [] };
  const leaf = (id, tabs) => ({ kind: 'leaf', id, tabs, activeTabId: tabs[0] });
  const sessions = [
    { id: 'demo-dev', name: 'w1.dev-server', agent: 'terminal' },
    { id: 'demo-claude', name: 'w1.interface', agent: 'claude' },
    { id: 'demo-codex', name: 'w1.review', agent: 'codex' },
    { id: 'demo-docs', name: 'w2.notes', agent: 'terminal' },
  ].map(session => ({ ...session, shell: 'powershell.exe', cwd, cols: 100, rows: 30 }));
  const profiles = [
    { id: 'demo-claude-profile', agent: 'claude', label: 'Personal', configDir: 'C:\\profiles\\claude\\personal', createdAt: 0 },
    { id: 'demo-codex-profile', agent: 'codex', label: 'Work', configDir: 'C:\\profiles\\codex\\work', createdAt: 0 },
  ];
  const snapshots = Object.fromEntries(sessions.slice(0, 3).map((s, i) => [s.id, {
    name: s.name.replace(/^w\d+\./, ''), terminal: shell, cwd,
    ...(i ? { accountProfile: { agent: profiles[i - 1].agent, id: profiles[i - 1].id, label: profiles[i - 1].label } } : {}),
  }]));
  localStorage.setItem('winmux:workspaces:v1', JSON.stringify({ version: 1, store: {
    activeWorkspaceId: 'demo-project', workspaces: [
      { id: 'demo-project', name: 'rhyme-terminal', index: 1, nextSessionSeq: 4, settings: { defaultCwd: cwd, terminal: null }, terminalSnapshots: snapshots,
        layout: { kind: 'split', id: 'root', direction: 'horizontal', sizes: [53, 47], children: [leaf('dev', ['demo-dev']), { kind: 'split', id: 'agents', direction: 'vertical', sizes: [50, 50], children: [leaf('claude', ['demo-claude']), leaf('codex', ['demo-codex'])] }] } },
      { id: 'demo-docs-project', name: 'documentation', index: 2, nextSessionSeq: 2, settings: { defaultCwd: 'C:\\projects\\docs', terminal: null }, terminalSnapshots: {}, layout: leaf('docs', ['demo-docs']) },
    ],
  } }));
  localStorage.setItem('winmux:prefs:v1', JSON.stringify({ version: 1, prefs: {
    language: 'ko', showAccountProfile: true,
    panels: { left: { open: true, width: 260 }, right: { open: true, width: 226 } },
    defaultProfileId: { claude: profiles[0].id, codex: profiles[1].id },
  } }));
  localStorage.setItem('winmux:accounts:v1', JSON.stringify({ version: 1, items: profiles }));
  const color = (code, text) => '\x1b[' + code + 'm' + text + '\x1b[0m';
  const logs = {
    'demo-dev': [
      '', color('1;36', '  RHYME TERMINAL'), color('90', '  A workspace for your terminal workflow.'), '',
      color('32', '  PS C:\\projects\\rhyme-terminal> ') + 'pnpm dev', '',
      '  > rhyme-terminal@0.1.0 dev', '  > vite', '',
      color('1;35', '  VITE') + '  development server', '',
      '  Local:   ' + color('36', 'http://localhost:3000/'), '  Network: use --host to expose', '',
      color('90', '  ------------------------------------------'), '',
      color('1', '  ONE PROJECT. THREE TERMINALS.'), '',
      '  dev-server    Preview the app', '  interface     Work with Claude Code', '  review        Review with Codex', '',
      color('90', '  ------------------------------------------'), '',
      '  Ctrl+N              New terminal', '  Ctrl+B, %           Split left / right', '  Ctrl+P              Find a file', '  Ctrl+,              Open settings', '',
      color('90', '  Example workspace for the README.'), '',
    ],
    'demo-claude': [
      '', color('1;33', '  CLAUDE CODE') + color('90', '  /  Personal'), '',
      color('90', '  C:\\projects\\rhyme-terminal'), '',
      color('36', '  > Refine the session navigation.'), '',
      '  Context', '  - Sidebar: projects and sessions', '  - Tabs: drag, split, rename', '  - Profiles: explicit per-session labels', '',
      color('90', '  Demo transcript · no agent request sent.'), '',
    ],
    'demo-codex': [
      '', color('1;35', '  CODEX') + color('90', '  /  Work'), '',
      color('90', '  C:\\projects\\rhyme-terminal'), '',
      color('36', '  > Review the workspace changes.'), '',
      '  Review checklist', '  [ ] Session ownership', '  [ ] Keyboard navigation', '  [ ] Layout persistence', '',
      color('90', '  Demo transcript · no agent request sent.'), '',
    ],
  };
  const node = (id, kind, config = {}) => ({ id, kind, config, input_bindings: {}, execution_policy: { timeout_secs: 60 } });
  const demoFlow = {
    schema_version: 1, flow_id: 'demo-flow', revision: 1, name: '검증 · 승인 · 결과 정리', inputs: { type: 'object' }, outputs: {},
    nodes: [node('check-types', 'command', { program: 'pnpm.cmd', args: ['exec', 'vue-tsc', '--noEmit'] }), node('run-tests', 'command', { program: 'pnpm.cmd', args: ['test'] }), node('build-app', 'command', { program: 'pnpm.cmd', args: ['build'] }), node('review', 'approval'), node('summary', 'output')],
    edges: [['check-types', 'build-app'], ['run-tests', 'build-app'], ['build-app', 'review'], ['review', 'summary']].map(([source, target], i) => ({ id: 'edge-' + i, source, target, source_port: 'success', target_port: 'input' })),
    policies: { concurrency: 2, timeout_secs: 600, capabilities: ['command'] },
    layout: { 'check-types': { x: 40, y: 50 }, 'run-tests': { x: 40, y: 235 }, 'build-app': { x: 305, y: 140 }, review: { x: 590, y: 140 }, summary: { x: 590, y: 330 } },
  };
  let callback = 0;
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    transformCallback: () => ++callback, unregisterCallback: () => {},
    invoke: async (command, args = {}) => {
      if (command === 'list_sessions') return sessions;
      if (command === 'get_account_usage') return [];
      if (command === 'attach_session') return btoa(String.fromCharCode(...new TextEncoder().encode((logs[args.id] || ['']).join('\r\n'))));
      if (command === 'flow_request') return { workers: [], flows: [demoFlow], tasks: [], runs: [] };
      if (command === 'plugin:path|resolve_directory') return 'C:\\Users\\demo';
      if (command === 'read_directory') return { path: args.path, entries: args.path === cwd ? ['docs', 'public', 'src', 'src-tauri', 'package.json', 'pnpm-lock.yaml', 'README.md', 'vite.config.ts'].map(name => ({ name, path: cwd + '\\' + name, isDir: !name.includes('.'), hidden: false })) : [] };
      if (command === 'list_files') return { root: cwd, files: [] };
      if (command === 'plugin:event|listen') return ++callback;
      if (command.includes('is_')) return false;
      if (['resize_session', 'detach_session', 'plugin:event|unlisten', 'plugin:window|start_dragging'].includes(command)) return null;
      return null;
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
}
