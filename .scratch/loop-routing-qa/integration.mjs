import fs from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
import http from 'node:http';
const staticRoot=path.resolve('dist');
const server=http.createServer(async(req,res)=>{try{const url=new URL(req.url,'http://localhost');const file=path.join(staticRoot,url.pathname==='/'?'index.html':url.pathname);if(!file.startsWith(staticRoot+path.sep)){res.writeHead(403);res.end();return;}const data=await fs.readFile(file);res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.ttf':'font/ttf'})[path.extname(file)]??'application/octet-stream');res.end(data);}catch{res.writeHead(404);res.end();}});
await new Promise(resolve=>server.listen(43359,'127.0.0.1',resolve));
const dir=path.resolve('.scratch/loop-routing-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
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
  const qaStore=JSON.parse(localStorage.getItem('winmux:workspaces:v1'));qaStore.store.workspaces.push({id:'qa-other',name:'Other',index:2,nextSessionSeq:1,settings:{defaultCwd:'C:/qa',terminal:null},layout:{kind:'leaf',id:'other-leaf',tabs:[],activeTabId:null},terminalSnapshots:{}});localStorage.setItem('winmux:workspaces:v1',JSON.stringify(qaStore));
 window.confirm=()=>true;
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(command,args={})=>{
 window.__qaCalls.push({command,args});
 if(command==='loop_request'){
 const r=args.request;
 if(r.op==='configure')return r.settings;
 if(r.op==='list')return JSON.parse(JSON.stringify(window.__groups));
 if(r.op==='create'){const g={id:'g'+(window.__groups.length+1),name:r.name,workspaceId:r.workspaceId,cwd:r.cwd,status:'running',activeSessionId:'p1',reason:null,updatedAt:1,attempts:[{id:'a1',sessionId:'p1',agent:'claude',profileId:'qa-claude',label:'Work Claude',status:'running',reason:null,startedAt:1,endedAt:null}]};window.__groups.push(g);return g;}
 if(r.op==='history')return btoa('Completed attempt output - read only');
 const g=window.__groups.find(g=>g.id===r.id);
 if(r.op==='move')g.workspaceId=r.workspaceId;
 if(r.op==='pause')g.status='paused';
 if(r.op==='resume')g.status='running';
 if(r.op==='stop'){g.status='stopped';g.activeSessionId=null;g.attempts.at(-1).endedAt=3;}
 if(r.op==='next'){g.attempts[0].endedAt=2;g.attempts[0].status='completed';g.activeSessionId='p2';g.attempts.push({id:'a2',sessionId:'p2',agent:'codex',profileId:'qa-codex',label:'Work Codex',status:'running',reason:null,startedAt:2,endedAt:null});}return g;
 }
 if(command==='list_sessions')return [session,{...session,id:'p1',name:'w1.managed',agent:'claude'}];if(command==='create_session')return session;if(command==='attach_session')return btoa('Loop QA terminal');if(command==='flow_request')return {workers:[],flows:[],tasks:[],runs:[]};if(command==='read_directory')return {path:args.path,entries:[]};if(command==='plugin:event|listen')return ++callback;if(command.includes('is_'))return false;if(command==='list_files')return {root:args.root,files:[]};return null;}};
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
 await click('.settings-shell .back-to-app');await click('.terminal-menu-toggle');
 await evaluate("[...document.querySelectorAll('.terminal-option')].find(x=>x.textContent.includes('새 에이전트 루프')).click()");await delay(1000);
 if(!await evaluate("!!document.querySelector('.loop-view .xterm')"))throw Error('Group terminal absent');
 if(!await evaluate("document.querySelector('.loop-view .term-host').getBoundingClientRect().top >= document.querySelector('.loop-toolbar').getBoundingClientRect().bottom"))throw Error('Live terminal covers loop toolbar');
 await shot('running');
 const tabBefore=await evaluate("document.querySelector('.tab.active .name').textContent");
 await evaluate("[...document.querySelectorAll('.loop-toolbar button')].find(x=>x.textContent.includes('다음 계정')).click()");await delay(700);
 const tabAfter=await evaluate("document.querySelector('.tab.active .name').textContent");
 if(tabBefore!==tabAfter)throw Error('Group tab identity changed');
 if(!await evaluate("document.querySelector('.loop-tag').textContent.includes('Work Codex')"))throw Error('Profile did not update');
 if(await evaluate("document.querySelector('.loop-toolbar select').options.length")!==2)throw Error('Only ended attempts should be selectable');
 await evaluate("const s=document.querySelector('.loop-toolbar select');s.value='a1';s.dispatchEvent(new Event('change',{bubbles:true}))");await delay(600);await shot('history');
 if(!await evaluate("!!document.querySelector('.history .xterm')"))throw Error('Read-only history absent');
 await evaluate("[...document.querySelectorAll('.loop-toolbar button')].find(x=>x.textContent==='일시정지').click()");await delay(350);
 if(!await evaluate("document.querySelector('.loop-view').textContent.includes('입력과 자동 전환이 잠깁니다')"))throw Error('Pause input guard explanation missing');
 await shot('paused');
 await evaluate("[...document.querySelectorAll('.sidebar .session')].find(x=>x.textContent.trim()==='qa').click()");await delay(200);
 if(await evaluate("!!document.querySelector('.loop-view')"))throw Error('Ordinary sidebar focus failed');
 await evaluate("[...document.querySelectorAll('.sidebar .session')].find(x=>x.textContent.includes('루프')).click()");await delay(200);
 if(!await evaluate("document.querySelector('.sidebar .session.focused').textContent.includes('루프')"))throw Error('Group sidebar focus failed');
 await shot('navigator');
 await evaluate("document.querySelector('.sidebar .ws').dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,clientX:100,clientY:100}))");await delay(200);
 await evaluate("document.querySelectorAll('.sidebar .menu .menu-item')[1].click()");await delay(700);
 if(!await evaluate("window.__groups[0].workspaceId==='qa-other'"))throw Error('Workspace group move missing');
 await evaluate("[...document.querySelectorAll('.sidebar .session')].find(x=>x.textContent.includes('루프')).click()");await delay(300);
 if(await evaluate("document.querySelectorAll('.sidebar .session').length")!==1)throw Error('Managed PTY leaked into navigator');
 if(!await evaluate("document.querySelector('.loop-view .term-host').getBoundingClientRect().top >= document.querySelector('.loop-toolbar').getBoundingClientRect().bottom"))throw Error('Moved terminal covers loop toolbar');
 await shot('workspace-moved');
 await evaluate("window.dispatchEvent(new KeyboardEvent('keydown',{key:'w',code:'KeyW',ctrlKey:true,bubbles:true,cancelable:true}))");await delay(500);
 if(!await evaluate("window.__groups[0].status==='stopped'"))throw Error('Ctrl+W did not stop group');
 if(await evaluate("[...document.querySelectorAll('.sidebar .session')].some(x=>x.textContent.includes('루프'))"))throw Error('Closed group still in navigator');
 await click('.terminal-menu-toggle');
 await evaluate("[...document.querySelectorAll('.terminal-option')].find(x=>x.textContent.includes('새 에이전트 루프')).click()");await delay(600);
 await evaluate("[...document.querySelectorAll('.sidebar .session')].find(x=>x.textContent.includes('루프')).dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,clientX:100,clientY:150}))");await delay(200);
 await click('.sidebar .menu .menu-item');
 await click('.modal .btn.primary');await delay(300);
 if(!await evaluate("window.__groups[1].status==='stopped'"))throw Error('Sidebar close did not stop group');
 const results={defaults,tabBefore,tabAfter,errors,loopCalls:await evaluate("window.__qaCalls.filter(x=>x.command==='loop_request').map(x=>x.args.request.op)")};
 await fs.writeFile(path.join(dir,'report.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));
 if(errors.length)throw Error('Runtime errors');
}finally{socket?.close();chrome.kill();server.close();}