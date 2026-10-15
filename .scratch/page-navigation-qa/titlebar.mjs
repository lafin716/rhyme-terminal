import fs from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
const dir=path.resolve('.scratch/page-navigation-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
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
 window.__qaCalls=[];window.__qaCancel=false;let callback=0;
 const session={id:'qa-session',name:'w1.qa',shell:'powershell.exe',cwd:'C:\\\\qa-project',cols:100,rows:24,agent:'terminal'};
 localStorage.setItem('winmux:workspaces:v1',JSON.stringify({version:1,store:{activeWorkspaceId:'qa-project',workspaces:[{id:'qa-project',name:'QA Project',index:1,nextSessionSeq:2,settings:{defaultCwd:'C:\\\\qa-project',terminal:null},layout:{kind:'leaf',id:'qa-leaf',tabs:['qa-session'],activeTabId:'qa-session'},terminalSnapshots:{}}]}}));
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(command,args={})=>{window.__qaCalls.push({command,args});if(command==='list_sessions')return [session];if(command==='create_session')return session;if(command==='attach_session')return btoa('PS C:\\\\qa-project> ');if(command==='flow_request')return {workers:[],flows:[],tasks:[],runs:[]};if(command==='read_directory')return {path:args.path,entries:[]};if(command==='plugin:dialog|save')return window.__qaCancel?null:'C:\\\\qa-project\\\\qa-note.txt';if(command==='plugin:event|listen')return ++callback;if(command.includes('is_'))return false;if(command==='list_files')return {root:args.root,files:[]};return null;}};
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 `});
 await cdp('Page.navigate',{url:'http://127.0.0.1:43309/'});
 for(let i=0;i<100;i++){if(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))break;await delay(500);}
 await delay(1500);

 const results={};
 const layout=async(page=null)=>evaluate(`(()=>{const r=s=>{const e=document.querySelector(s);if(!e)return null;const b=e.getBoundingClientRect();return {x:b.x,y:b.y,width:b.width,height:b.height,topHit:document.elementFromPoint(b.x+b.width/2,b.y+b.height/2)?.closest(s)===e}};return {tab:r('.tab'),controls:r('.window-controls'),logo:r('.app-icon'),page:${page?`r('${page}')`:'null'},terminalPreserved:!!window.__qaTerminal&&window.__qaTerminal===document.querySelector('.term-host'),terminalScreen:!!document.querySelector('.xterm-screen'),scrollWidth:document.documentElement.scrollWidth,width:innerWidth}})()`);
 await evaluate("window.__qaTerminal=document.querySelector('.term-host')");
 for(const width of [1440,800]){
  await cdp('Emulation.setDeviceMetricsOverride',{width,height:width===1440?1000:700,deviceScaleFactor:1,mobile:false});await delay(400);
  results[width]={terminal:await layout()};await shot(`titlebar-${width}-terminal`);
  await evaluate("document.querySelector('.open-flow').click()");await delay(400);
  results[width].flow=await layout('.flow-page');await shot(`titlebar-${width}-flow`);
  await evaluate("document.querySelectorAll('.flow-page .nav-item')[1].click()");await delay(200);
  results[width].workerNav=await evaluate("!!document.querySelector('.flow-panel .worker-layout')&&document.querySelectorAll('.flow-page .nav-item')[1].classList.contains('active')");
  await evaluate("document.querySelectorAll('.flow-page .nav-item')[0].click()");await delay(150);
  await evaluate("document.querySelector('.flow-page .back-to-app').click()");await delay(200);
  results[width].flowReturned=await evaluate("getComputedStyle(document.querySelector('.flow-page')).display==='none'");
  await evaluate("document.querySelector('.settings-button').click()");await delay(400);
  results[width].settings=await layout('.settings-shell');await shot(`titlebar-${width}-settings`);
  await evaluate("document.querySelector('.settings-shell .back-to-app').click()");await delay(200);
  results[width].settingsReturned=await evaluate("!document.querySelector('.settings-shell')");
  results[width].after=await layout();
  await evaluate("document.querySelector('.open-flow').click()");await delay(200);
  await evaluate("document.querySelector('.overflow-menu .corner-toggle').click()");await delay(150);
  results[width].menuHit=await evaluate("(()=>{const e=document.querySelectorAll('.group-item')[4];const b=e.getBoundingClientRect();return document.elementFromPoint(b.x+b.width/2,b.y+b.height/2)?.closest('.group-item')===e})()");
  await evaluate("document.querySelectorAll('.group-item')[4].click()");await delay(150);
  await evaluate("document.querySelector('.submenu .item').click()");await delay(250);
  results[width].menuSettings=await evaluate("!!document.querySelector('.settings-shell')&&getComputedStyle(document.querySelector('.flow-page')).display==='none'");
  await evaluate("document.querySelector('.settings-shell .back-to-app').click()");await delay(150);
 }
 results.errors=errors;
 await fs.writeFile(path.join(dir,'titlebar-report.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));
 for(const r of [results[1440],results[800]]){
  if(r.terminal.tab.y<0||r.terminal.tab.y+r.terminal.tab.height>36||r.terminal.controls.y!==0||r.terminal.tab.height>36||r.terminal.controls.height!==36)throw Error('Terminal tab/window controls row mismatch');
  for(const p of [r.flow,r.settings])if(p.page.y!==36||!p.controls.topHit||!p.logo.topHit||!p.terminalPreserved)throw Error('Page blocks titlebar or remounts terminal');
  if(!r.workerNav||!r.menuHit||!r.menuSettings)throw Error('Sidebar nav or application menu navigation failed');
  if(!r.flowReturned||!r.settingsReturned||!r.after.terminalPreserved||!r.after.terminalScreen)throw Error('Back navigation failed');
 }
 if(errors.length)throw Error('Runtime errors');
}finally{socket?.close();chrome.kill();}


