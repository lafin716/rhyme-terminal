// Run after pnpm.cmd build: node scripts/qa-new-file.mjs [--fail-editor]
// Uses isolated headless Chrome and mocked Tauri APIs; no real files or sessions are changed.
import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import {spawn} from 'node:child_process';
const dir=path.resolve('.scratch/page-navigation-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
await fs.mkdir(profile,{recursive:true});
const chrome=spawn('C:/Program Files/Google/Chrome/Application/chrome.exe',['--headless=new','--disable-gpu','--no-first-run','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'],{windowsHide:true,stdio:'ignore'});
const root = path.resolve('dist');
const server = http.createServer(async (req,res) => {
 if(process.argv.includes('--fail-editor') && req.url.includes('/assets/FileViewer-') && req.url.endsWith('.js')){res.writeHead(503).end();return;}
 const file=path.resolve(root,'.'+(new URL(req.url,'http://localhost').pathname==='/'?'/index.html':new URL(req.url,'http://localhost').pathname));
 if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}
 try { const data=await fs.readFile(file); res.setHeader('Content-Type',file.endsWith('.js')?'application/javascript':file.endsWith('.css')?'text/css':file.endsWith('.html')?'text/html':'application/octet-stream');res.end(data); } catch {res.writeHead(404).end();}
});
await new Promise(r=>server.listen(43319,'127.0.0.1',r));
let socket;
const delay=ms=>new Promise(r=>setTimeout(r,ms));
try {
 let port;for(let i=0;i<300;i++){try{port=Number((await fs.readFile(path.join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0]);break;}catch{await delay(200);}}
 if(!port)throw Error('No Chrome endpoint');
 const tabs=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();socket=new WebSocket(tabs.find(t=>t.type==='page').webSocketDebuggerUrl);await new Promise((r,j)=>{socket.onopen=r;socket.onerror=j;});
 let seq=0;const pending=new Map(),errors=[];
 socket.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);pending.delete(m.id);m.error?p?.reject(m.error):p?.resolve(m.result);}if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails);};
 const cdp=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));});
 const evaluate=async expression=>{const r=await cdp('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result?.value;};
 const shot=async name=>{const r=await cdp('Page.captureScreenshot',{format:'png'});await fs.writeFile(path.join(dir,name+'.png'),Buffer.from(r.data,'base64'));};
 await cdp('Page.enable');await cdp('Runtime.enable');await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
 await cdp('Page.addScriptToEvaluateOnNewDocument',{source:`
 window.__qaCalls=[];window.__qaCancel=false;let callback=0;
 const session={id:'qa-session',name:'w1.qa',shell:'powershell.exe',cwd:'C:\\\\qa-project',cols:100,rows:24,agent:'terminal'};
 localStorage.setItem('winmux:workspaces:v1',JSON.stringify({version:1,store:{activeWorkspaceId:'qa-project',workspaces:[{id:'qa-project',name:'QA Project',index:1,nextSessionSeq:2,settings:{defaultCwd:'C:\\\\qa-project',terminal:null},layout:{kind:'leaf',id:'qa-leaf',tabs:['qa-session'],activeTabId:'qa-session'},terminalSnapshots:{}}]}}));
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(command,args={})=>{window.__qaCalls.push({command,args});if(command==='list_sessions')return [session];if(command==='create_session')return session;if(command==='attach_session')return btoa('PS C:\\\\qa-project> ');if(command==='flow_request')return {workers:[],flows:[],tasks:[],runs:[]};if(command==='read_directory')return {path:args.path,entries:[]};if(command==='plugin:dialog|save')return window.__qaCancel?null:'C:\\\\qa-project\\\\qa-note.txt';if(command==='plugin:event|listen')return ++callback;if(command.includes('is_'))return false;if(command==='list_files')return {root:args.root,files:[]};return null;}};
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 `});
 await cdp('Page.navigate',{url:'http://127.0.0.1:43319/'});
 for(let i=0;i<100;i++){if(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))break;await delay(500);}
 await delay(1500);
 await evaluate("document.querySelector('.terminal-menu-toggle').click()");await delay(100);await evaluate("[...document.querySelectorAll('.terminal-option')].find(e=>/새 페이지|새 파일|Open new page|New File/.test(e.textContent)).click()");await delay(7000);
 const check=async(expression,message)=>{if(!await evaluate(expression))throw Error(message);};
 await check("!!document.querySelector('.monaco-editor, .fallback-editor')",'New file has no usable editor');
 if(process.argv.includes('--fail-editor')) {
   await check("!!document.querySelector('.fallback-editor')",'Fallback editor missing');
   await evaluate("(()=>{const e=document.querySelector('.fallback-editor');e.value='draft after load failure';e.dispatchEvent(new Event('input',{bubbles:true}));})()");
 } else {
   await check("!!document.querySelector('.monaco-editor')&&!document.querySelector('.viewer-status')",'Monaco did not finish loading');
   await cdp('Input.insertText',{text:'draft after load failure'});
 }
 await evaluate("window.__qaCancel=true;document.querySelector('.file-viewer button, .fallback-toolbar button').click()");await delay(100);
 await check("!__qaCalls.some(c=>c.command==='write_file')",'Cancel wrote a file');
 await evaluate("window.__qaCancel=false;document.querySelector('.file-viewer button, .fallback-toolbar button').click()");await delay(200);
 await check("__qaCalls.some(c=>c.command==='write_file'&&c.args.contents==='draft after load failure')",'Draft was not saved');
 await evaluate("document.querySelector('.terminal-menu-toggle').click()");await delay(100);
 await check("[...document.querySelectorAll('.terminal-option')].some(e=>e.textContent.trim()==='새 파일')",'New File label missing');
 await evaluate("document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))");
 if(errors.length)throw Error(JSON.stringify(errors));
 await shot(process.argv.includes('--fail-editor')?'new-file-fallback':'new-file-normal');
 console.log('PASS: New File opens, edits, cancels save, and saves '+(process.argv.includes('--fail-editor')?'despite module failure':'with Monaco'));
}finally{socket?.close();chrome.kill();server.close();}

