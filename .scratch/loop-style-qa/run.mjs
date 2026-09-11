import fs from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
import http from 'node:http';
const staticRoot=path.resolve('dist');
const server=http.createServer(async(req,res)=>{try{const url=new URL(req.url,'http://localhost');const file=path.join(staticRoot,url.pathname==='/'?'index.html':url.pathname);if(!file.startsWith(staticRoot+path.sep)){res.writeHead(403);res.end();return;}const data=await fs.readFile(file);res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.ttf':'font/ttf'})[path.extname(file)]??'application/octet-stream');res.end(data);}catch{res.writeHead(404);res.end();}});
await new Promise(resolve=>server.listen(43359,'127.0.0.1',resolve));
const dir=path.resolve('.scratch/loop-style-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
await fs.mkdir(profile,{recursive:true});
const chrome=spawn('C:/Program Files/Google/Chrome/Application/chrome.exe',['--headless=new','--disable-gpu','--no-first-run','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'],{windowsHide:true,stdio:'ignore'});
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
 window.__qaCalls=[];let callback=0;window.__groups=[];
 const session={id:'qa-session',name:'w1.qa',shell:'powershell.exe',cwd:'C:/qa',cols:100,rows:24,agent:'terminal'};
 localStorage.setItem('winmux:accounts:v1',JSON.stringify({version:1,items:[{id:'qa-claude',agent:'claude',label:'Work Claude',configDir:'C:/qa/claude',createdAt:1},{id:'qa-codex',agent:'codex',label:'Work Codex',configDir:'C:/qa/codex',createdAt:1}]}));
 localStorage.setItem('winmux:workspaces:v1',JSON.stringify({version:1,store:{activeWorkspaceId:'qa-project',workspaces:[{id:'qa-project',name:'QA',index:1,nextSessionSeq:2,settings:{defaultCwd:'C:/qa',terminal:null},layout:{kind:'leaf',id:'qa-leaf',tabs:['qa-session'],activeTabId:'qa-session'},terminalSnapshots:{}}]}}));
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(command,args={})=>{
 window.__qaCalls.push({command,args});
 if(command==='loop_request'){
 const r=args.request;
 if(r.op==='configure')return r.settings;
 if(r.op==='list')return JSON.parse(JSON.stringify(window.__groups));
 if(r.op==='create'){const g={id:'g1',name:r.name,workspaceId:r.workspaceId,cwd:r.cwd,status:'running',activeSessionId:'p1',reason:null,updatedAt:1,attempts:[{id:'a1',sessionId:'p1',agent:'claude',profileId:'qa-claude',label:'Work Claude',status:'running',reason:null,startedAt:1,endedAt:null}]};window.__groups.push(g);return g;}
 if(r.op==='history')return btoa('Completed attempt output - read only');
 const g=window.__groups.find(g=>g.id===r.id);
 if(r.op==='pause')g.status='paused';
 if(r.op==='resume')g.status='running';
 if(r.op==='stop'){g.status='stopped';g.activeSessionId=null;g.attempts.at(-1).endedAt=3;}
 if(r.op==='next'){g.attempts[0].endedAt=2;g.attempts[0].status='completed';g.activeSessionId='p2';g.attempts.push({id:'a2',sessionId:'p2',agent:'codex',profileId:'qa-codex',label:'Work Codex',status:'running',reason:null,startedAt:2,endedAt:null});}return g;
 }
 if(command==='list_sessions')return [session];if(command==='create_session')return session;if(command==='attach_session')return btoa('Loop QA terminal');if(command==='flow_request')return {workers:[],flows:[],tasks:[],runs:[]};if(command==='read_directory')return {path:args.path,entries:[]};if(command==='plugin:event|listen')return ++callback;if(command.includes('is_'))return false;if(command==='list_files')return {root:args.root,files:[]};return null;}};
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 `});
 await cdp('Page.navigate',{url:'http://127.0.0.1:43359/'});
 for(let i=0;i<240;i++){if(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))break;await delay(300);}
 await delay(1500);
 const click=async selector=>{await evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);await delay(350);};
 await shot('initial');
 console.log('INITIAL',await evaluate('document.body.innerText.slice(0,2000)'),JSON.stringify(errors));
 await click('.settings-button');
 await evaluate("[...document.querySelectorAll('.settings-shell .nav-item')].find(x=>x.textContent.includes('에이전트 루프')).click()");await delay(350);
 const defaults=await evaluate("[...document.querySelectorAll('.loop-settings input[type=checkbox]')].map(x=>x.checked)");
 if(defaults.length!==4||defaults.some(Boolean))throw Error('Candidates must be excluded initially');
 await click('.loop-settings input[type=checkbox]');await click('.loop-settings button[type=submit]');await shot('settings');
 const desktop = await evaluate(`({titleSize:getComputedStyle(document.querySelector('.loop-settings h2')).fontSize,navIcon:!!document.querySelector('.nav-item.active .nav-icon svg'),navDescription:!!document.querySelector('.nav-item.active .nav-desc'),saved:document.querySelector('.loop-settings [role=status]').textContent})`);
 if(desktop.titleSize!=='20px'||!desktop.navIcon||!desktop.navDescription)throw Error('Settings design mismatch');
 await cdp('Emulation.setDeviceMetricsOverride',{width:700,height:900,deviceScaleFactor:1,mobile:false});await delay(350);await shot('narrow');
 const overflow=await evaluate(`document.querySelector('.loop-settings').scrollWidth > document.querySelector('.loop-settings').clientWidth`);
 if(overflow)throw Error('Horizontal overflow');
 if(errors.length)throw Error(JSON.stringify(errors));
 console.log(JSON.stringify({desktop,overflow,errors}));
}finally{socket?.close();chrome.kill();server.close();}