import fs from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';

const dir = path.resolve('.scratch/terminal-menu-qa');
const profile = path.join(dir, `chrome-${Date.now()}`);
await fs.mkdir(profile, { recursive: true });
const chrome = spawn('C:/Program Files/Google/Chrome/Application/chrome.exe',
  ['--headless=new', '--disable-gpu', '--no-first-run', '--remote-debugging-port=0',
    `--user-data-dir=${profile}`, 'about:blank'], { windowsHide: true, stdio: 'ignore' });
let socket;
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
try {
  let port;
  for (let i = 0; i < 300; i++) {
    try { port = Number((await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]); break; }
    catch { await delay(200); }
  }
  if (!port) throw Error('No Chrome endpoint');
  const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  socket = new WebSocket(tabs.find((t) => t.type === 'page').webSocketDebuggerUrl);
  await new Promise((r, j) => { socket.onopen = r; socket.onerror = j; });
  let seq = 0; const pending = new Map(); const errors = [];
  socket.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id) { const p = pending.get(m.id); pending.delete(m.id); m.error ? p?.reject(m.error) : p?.resolve(m.result); }
    if (m.method === 'Runtime.exceptionThrown') errors.push(m.params.exceptionDetails);
  };
  const cdp = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++seq;
    const timer = setTimeout(() => reject(Error('Timeout: ' + method)), 60000);
    pending.set(id, { resolve: (v) => { clearTimeout(timer); resolve(v); }, reject: (e) => { clearTimeout(timer); reject(e); } });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => {
    const r = await cdp('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw Error(expression + ' -> ' + JSON.stringify(r.exceptionDetails.exception?.description));
    return r.result?.value;
  };
  const shot = async (name) => {
    const r = await cdp('Page.captureScreenshot', { format: 'png' });
    await fs.writeFile(path.join(dir, name + '.png'), Buffer.from(r.data, 'base64'));
  };
  await cdp('Page.enable'); await cdp('Runtime.enable');
  await cdp('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
  await cdp('Page.addScriptToEvaluateOnNewDocument', { source: `
 window.__qaCalls=[];let callback=0;
 const session={id:'qa-session',name:'w1.qa',shell:'powershell.exe',cwd:'C:\\\\qa-project',cols:100,rows:24,agent:'terminal'};
 localStorage.setItem('winmux:accounts:v1',JSON.stringify({version:1,items:[{id:'acc-work',agent:'claude',label:'Work',configDir:'C:\\\\qa\\\\work',createdAt:1},{id:'acc-home',agent:'claude',label:'Home',configDir:'C:\\\\qa\\\\home',createdAt:2}]}));
 localStorage.setItem('winmux:prefs:v1',JSON.stringify({version:1,prefs:{defaultProfileId:{claude:'acc-home',codex:null}}}));
 localStorage.setItem('winmux:workspaces:v1',JSON.stringify({version:1,store:{activeWorkspaceId:'qa-project',workspaces:[{id:'qa-project',name:'QA Project',index:1,nextSessionSeq:2,settings:{defaultCwd:'C:\\\\qa-project',terminal:null},layout:{kind:'leaf',id:'qa-leaf',tabs:['qa-session'],activeTabId:'qa-session'},terminalSnapshots:{}}]}}));
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(command,args={})=>{window.__qaCalls.push({command,args});if(command==='list_sessions')return [session];if(command==='create_session')return {...session,id:'qa-session-'+window.__qaCalls.length};if(command==='attach_session')return btoa('PS C:\\\\qa-project> ');if(command==='flow_request')return {workers:[],flows:[],tasks:[],runs:[]};if(command==='read_directory')return {path:args.path,entries:[]};if(command==='plugin:event|listen')return ++callback;if(command.includes('is_'))return false;if(command==='list_files')return {root:args.root,files:[]};return null;}};
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 ` });
  cdp('Page.navigate', { url: 'http://localhost:43411/' }).catch(() => {});
  for (let i = 0; i < 100; i++) {
    if (await evaluate("!!document.querySelector('.terminal-menu-toggle')")) break;
    await delay(500);
  }
  if (!(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))) throw Error('App never rendered');
  await delay(1500);

  const results = {};
  const rows = () => evaluate("Array.from(document.querySelectorAll('.terminal-picker:not(.profile-picker) > .terminal-option')).map(e=>e.textContent.trim().replace(/\\s+/g,' '))");
  const submenuRows = () => evaluate("Array.from(document.querySelectorAll('.profile-picker > .terminal-option')).map(e=>e.textContent.trim().replace(/\\s+/g,' '))");
  const openMenu = async () => { await evaluate("document.querySelector('.terminal-menu-toggle').click()"); await delay(200); };
  // Split rows are matched by their visible label so the terminal row and the
  // agent rows never pick each other's buttons.
  const row = (label) => `Array.from(document.querySelectorAll('.terminal-picker > .split-option')).find(e=>e.textContent.includes('${label}'))`;
  const clickIn = (label, part) => evaluate(`${row(label)}.querySelector('.split-option-${part}').click()`);
  const pickSubmenu = (label) => evaluate(`Array.from(document.querySelectorAll('.profile-picker > .terminal-option')).find(e=>e.textContent.includes('${label}')).click()`);
  const createCalls = () => evaluate("window.__qaCalls.filter(c=>c.command==='create_session')");

  await openMenu();
  results.menu = await rows();
  await shot('menu');

  // Arrow next to "New Terminal" opens the shell picker.
  await clickIn('새 터미널', 'arrow');
  await delay(250);
  results.shellPicker = await submenuRows();
  await shot('shell-picker');

  // Plain click on the row launches the default shell.
  await clickIn('새 터미널', 'main');
  await delay(400);
  results.createdDefault = (await createCalls()).map((c) => c.args.shell ?? c.args.program ?? JSON.stringify(c.args));
  results.menuClosedAfterCreate = await evaluate("!document.querySelector('.terminal-picker')");

  // Picking a shell from the flyout launches that one instead.
  await openMenu();
  await clickIn('새 터미널', 'arrow');
  await delay(250);
  await pickSubmenu('명령 프롬프트');
  await delay(400);
  results.createCalls = (await createCalls()).length;

  // "New browser" row adds a browser tab.
  await openMenu();
  await evaluate("Array.from(document.querySelectorAll('.terminal-picker > .terminal-option')).find(e=>e.textContent.includes('브라우저')).click()");
  await delay(600);
  results.tabs = await evaluate("Array.from(document.querySelectorAll('.tab')).map(e=>e.textContent.trim().replace(/\\s+/g,' '))");
  await shot('after-browser');

  // Claude row: the arrow opens the account picker, a plain click runs the default account.
  await openMenu();
  results.claudeRow = await evaluate(`${row('Claude')}.textContent.trim().replace(/\\s+/g,' ')`);
  results.codexRowHasArrow = await evaluate(`!!${row('Codex')}.querySelector('.split-option-arrow')`);
  await clickIn('Claude', 'arrow');
  await delay(250);
  results.accountPicker = await submenuRows();
  await shot('account-picker');
  await clickIn('Claude', 'main');
  await delay(600);
  results.claudeLaunch = (await createCalls()).slice(-1).map((c) => ({ env: c.args.env, command: c.args.launchCommand ?? c.args.args }));

  results.errors = errors.map((e) => e.text + ' ' + (e.exception?.description ?? ''));
  await fs.writeFile(path.join(dir, 'report.json'), JSON.stringify(results, null, 2));
  console.log(JSON.stringify(results, null, 2));

  const fail = (m) => { throw Error(m); };
  if (!results.menu.some((r) => r.includes('새 터미널'))) fail('No consolidated New Terminal row');
  if (results.menu.some((r) => /Zsh|Git Bash|WSL|명령 프롬프트/.test(r))) fail('Per-shell rows still in the top-level menu');
  if (!results.menu.some((r) => r.includes('새 브라우저 열기'))) fail('No new browser row');
  if (results.shellPicker.some((r) => r.includes('Zsh'))) fail('Zsh offered on Windows');
  if (!results.shellPicker.some((r) => r.includes('Windows PowerShell'))) fail('Windows shells missing from picker');
  if (results.createdDefault.length !== 1) fail('Default click did not create exactly one session');
  if (results.createCalls !== 2) fail('Shell pick did not create a session');
  if (!results.tabs.some((tab) => tab.includes('google'))) fail('Browser tab not opened');
  if (!results.claudeRow.includes('Home')) fail('Claude row does not show its default account');
  if (results.codexRowHasArrow) fail('Codex row shows an arrow without profiles');
  if (!results.accountPicker.some((r) => r.includes('Work')) || !results.accountPicker.some((r) => r.includes('Home'))) fail('Account picker missing profiles');
  if (!/qa..home/.test(JSON.stringify(results.claudeLaunch))) fail('Plain Claude click did not launch the default account');
  if (results.errors.length) fail('Runtime errors: ' + JSON.stringify(results.errors));
  console.log('QA PASSED');
} finally {
  socket?.close();
  chrome.kill();
}
