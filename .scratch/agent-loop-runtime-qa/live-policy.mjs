import fs from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
import http from 'node:http';
const staticRoot=path.resolve('dist');
const server=http.createServer(async(req,res)=>{try{const url=new URL(req.url,'http://localhost');const file=path.join(staticRoot,url.pathname==='/'?'index.html':url.pathname);if(!file.startsWith(staticRoot+path.sep)){res.writeHead(403);res.end();return;}const data=await fs.readFile(file);res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.ttf':'font/ttf'})[path.extname(file)]??'application/octet-stream');res.end(data);}catch{res.writeHead(404);res.end();}});
await new Promise(resolve=>server.listen(43369,'127.0.0.1',resolve));
const dir=path.resolve('.scratch/agent-loop-runtime-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
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
 if(r.op==='capabilities')return {version:1,runtimeMonitor:true,autoStart:true,livePolicy:true};if(r.op==='configure'){window.__policy=JSON.parse(JSON.stringify(r.settings));return r.settings;}
 if(r.op==='list')return JSON.parse(JSON.stringify(window.__groups));
 if(r.op==='create'){const g={id:'g1',name:r.name,workspaceId:r.workspaceId,cwd:r.cwd,policy:JSON.parse(JSON.stringify(window.__policy)),status:'resuming',activeSessionId:'p1',reason:null,updatedAt:1,attempts:[],runtime:{status:'IDLE',provider:null,pid:null},activeProfile:null,events:[{at:Date.now(),kind:'CREATED',message:'Agent 대기 중'}],profiles:[{key:'claude:qa-claude',agent:'codex',label:'Work 01',status:'EXHAUSTED',usage:91,threshold:90,remaining:9,resetAt:Date.now()+3600000,error:null},{key:'codex:qa-codex',agent:'codex',label:'Work 02',status:'AVAILABLE',usage:43,threshold:90,remaining:57,resetAt:null,error:null}]};window.__groups.push(g);return g;}
 if(r.op==='history')return btoa('Completed attempt output - read only');
 const g=window.__groups.find(g=>g.id===r.id);
 if(r.op==='update_policy'){
 const p=r.patch.profile;
 if(p){if(p.key===g.activeProfile&&(p.shortThreshold!=null||p.weeklyThreshold!=null))throw Error('Active profile locked');const c=g.policy.candidates.find(c=>c.agent+':'+c.profileId===p.key);Object.assign(c,p);if(p.shortThreshold!=null)g.profiles.find(c=>c.key===p.key).threshold=p.shortThreshold;}
 else Object.assign(g.policy,r.patch);
 return JSON.parse(JSON.stringify(g));
 }

 if(r.op==='pause')g.status='paused';
 if(r.op==='resume')g.status='running';
 if(r.op==='stop'){g.status='stopped';g.activeSessionId=null;g.attempts.at(-1).endedAt=3;}
 if(r.op==='next'){g.attempts[0].endedAt=2;g.attempts[0].status='completed';g.activeSessionId='p2';g.attempts.push({id:'a2',sessionId:'p2',agent:'codex',profileId:'qa-codex',label:'Work Codex',status:'running',reason:null,startedAt:2,endedAt:null});}return g;
 }
 if(command==='list_sessions')return [session,...window.__groups.map(g=>({...session,id:g.activeSessionId}))];if(command==='create_session')return session;if(command==='attach_session')return btoa('Loop QA terminal');if(command==='flow_request')return {workers:[],flows:[],tasks:[],runs:[]};if(command==='read_directory')return {path:args.path,entries:[]};if(command==='plugin:event|listen')return ++callback;if(command.includes('is_'))return false;if(command==='list_files')return {root:args.root,files:[]};return null;}};
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 `});
 await cdp('Page.navigate',{url:'http://127.0.0.1:43369/'});
 for(let i=0;i<240;i++){if(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))break;await delay(300);}
 await delay(1500);
 const click=async selector=>{await evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);await delay(350);};
 const assert=async(expression,message)=>{if(!await evaluate(expression))throw Error(message);};
 await evaluate("document.dispatchEvent(new KeyboardEvent('keydown',{key:',',ctrlKey:true,bubbles:true}))");await delay(500);
 await evaluate("[...document.querySelectorAll('.nav-item')].find(x=>x.textContent.includes('에이전트 루프')).click()");await delay(300);
 await assert("document.querySelectorAll('.provider-columns').length===2",'Provider columns missing');
 await evaluate("[...document.querySelectorAll('.candidate')].filter(x=>x.textContent.includes('Work ')).forEach(x=>x.querySelector('.toggle-profile').click())");await delay(300);
 await assert("document.querySelectorAll('.profile-zone:first-child .candidate').length===2",'Activation failed');
 await evaluate("(()=>{const rows=[...document.querySelectorAll('.profile-zone:first-child .candidate')],dt=new DataTransfer();rows[1].querySelector('.drag-handle').dispatchEvent(new DragEvent('dragstart',{bubbles:true,dataTransfer:dt}));rows[0].dispatchEvent(new DragEvent('drop',{bubbles:true,dataTransfer:dt}));})()");await delay(300);
 await assert("[...document.querySelectorAll('.profile-zone:first-child .candidate')].find(x=>x.textContent.includes('Work Codex')).querySelector('.order').textContent==='1'",'Cross provider reorder failed');
 await evaluate("(()=>{const row=[...document.querySelectorAll('.profile-zone:first-child .candidate')].find(x=>x.textContent.includes('Work Claude')),dt=new DataTransfer();row.querySelector('.drag-handle').dispatchEvent(new DragEvent('dragstart',{bubbles:true,dataTransfer:dt}));document.querySelectorAll('.profile-zone')[1].dispatchEvent(new DragEvent('drop',{bubbles:true,dataTransfer:dt}));})()");await delay(250);
 await assert("document.querySelectorAll('.profile-zone:first-child .candidate').length===1",'Drag to waiting failed');
 await evaluate("[...document.querySelectorAll('.profile-zone:last-child .candidate')].find(x=>x.textContent.includes('Work Claude')).querySelector('.toggle-profile').click()");await delay(250);
 await shot('settings-profiles');
 await cdp('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:false});await delay(250);await shot('settings-profiles-narrow');
 await assert("document.querySelector('.profile-board').scrollWidth<=document.querySelector('.profile-board').clientWidth",'Settings board overflow');
 await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});

 await click('.loop-settings button[type=submit]');
 await assert("JSON.parse(localStorage.getItem('winmux:loop-routing:v1')).candidates.filter(c=>c.enabled)[0].agent==='codex'",'Order not persisted');
 await click('.back-to-app');
 await click('.terminal-menu-toggle');
 await evaluate("[...document.querySelectorAll('.terminal-option')].find(x=>x.textContent.includes('새 에이전트 루프')).click()");await delay(500);
 await assert("!!document.querySelector('dialog[open]')",'Creation dialog absent');
 await assert("!document.querySelector('#loop-prompt') && !document.querySelector('.mode') && !document.querySelector('.session-panel')",'Legacy creation controls remain');
 await assert("document.querySelectorAll('.loop-create fieldset .participant').length===2",'Waiting profiles leaked into create');await shot('create-desktop');
 await cdp('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:false});await delay(300);await shot('create-narrow');
 await assert("(()=>{const r=document.querySelector('dialog').getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&r.top>=0&&r.bottom<=innerHeight})()",'Dialog overflow');
 await click('.loop-create .primary');await delay(1200);
 await assert("window.__qaCalls.filter(x=>x.command==='loop_request'&&x.args.request.op==='create').every(x=>!('start' in x.args.request))",'Creation includes start mode');
 await assert("!!document.querySelector('.loop-view .term-host') && !document.querySelector('.loop-view .details')",'Terminal or collapsed panel missing');
 await assert("document.querySelector('.loop-toolbar').textContent.includes('작업 재개 중')",'Auto launch state missing');
 await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});await delay(300);await shot('idle');
 await evaluate("Object.assign(window.__groups[0],{status:'running',activeProfile:'codex:qa-codex',currentProvider:'codex',runtime:{status:'RUNNING',provider:'codex',pid:42}})");await evaluate("window.__groups[0].profiles[1].status='ACTIVE'");await delay(2500);await shot('running');
 await assert("document.querySelector('.active-profile').textContent.includes('Work 02') && !document.querySelector('.loop-toolbar select')",'Readonly active profile missing');
 await click('.loop-toolbar .expand');await shot('expanded');
 await assert("document.querySelectorAll('.profile.selected input:disabled').length===2",'Active thresholds are not locked');
 await evaluate("(()=>{const input=document.querySelector('.profile:not(.selected) input');input.value='75';input.dispatchEvent(new Event('change',{bubbles:true}));})()");await delay(500);
 await assert("window.__groups[0].policy.candidates.find(c=>c.agent==='claude'&&c.profileId==='qa-claude').shortThreshold===75",'Profile threshold not saved');
 await assert("document.querySelector('.profile:not(.selected) .meter i').title==='임계값 75%'",'Threshold graph did not update');
 await evaluate("(()=>{const select=document.querySelector('.session-policy select');select.value='PRIORITY';select.dispatchEvent(new Event('change',{bubbles:true}));})()");await delay(500);
 await assert("window.__groups[0].policy.strategy==='PRIORITY' && window.__policy.strategy==='SMART'",'Per loop strategy leaked into defaults');
 await evaluate("(()=>{const input=document.querySelector('.profile.selected input');input.value='99';input.dispatchEvent(new Event('change',{bubbles:true}));})()");await delay(500);
 await assert("document.querySelector('.profile.selected input').value==='90' && document.querySelector('.latest').textContent.includes('Active profile locked')",'Rejected threshold was not restored');
 await shot('live-policy');

 await assert("document.querySelectorAll('.profile [role=progressbar]').length===2",'Usage graphs missing');
 await evaluate("Object.assign(window.__groups[0],{status:'waiting_for_usage_reset',waitingProfileId:'claude:qa-claude',resumeAt:Date.now()+120000,runtime:{status:'IDLE',provider:null,pid:null}})");await delay(2500);await shot('waiting');
 await assert("!!document.querySelector('.reset time')",'Countdown missing');
 await cdp('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:false});await delay(350);await shot('panel-narrow');
 const overflow=await evaluate("document.querySelector('.loop-view').scrollWidth>document.querySelector('.loop-view').clientWidth");
 if(overflow)throw Error('Loop horizontal overflow');
 const result={errors,overflow,checks:['live-threshold-edit','active-threshold-lock','per-loop-strategy','no-session-selector','settings-columns','drag-reorder','active-only','auto-start-terminal','collapsed-by-default','active-profile','usage-bars','countdown','desktop-and-narrow']};
 await fs.writeFile(path.join(dir,'report.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
 if(errors.length)throw Error('Browser exceptions');
}finally{socket?.close();chrome.kill();server.close();}