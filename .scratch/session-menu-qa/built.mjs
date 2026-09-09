import fs from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
const dir=path.resolve('.scratch/session-menu-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
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
 const cdp=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>reject(Error('Timeout: '+method)),15000);pending.set(id,{resolve:v=>{clearTimeout(timer);resolve(v)},reject:e=>{clearTimeout(timer);reject(e)}});socket.send(JSON.stringify({id,method,params}));});
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
 console.log('Navigating');await cdp('Page.navigate',{url:'http://127.0.0.1:43329/'});
 for(let i=0;i<100;i++){if(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))break;await delay(500);}
 await delay(1500);


 console.log('Page ready');const results={};
 const menuLabels=()=>evaluate("Array.from(document.querySelectorAll('.terminal-picker:not(.profile-picker) > .terminal-option')).map(e=>e.textContent.trim().replace(/\\s+/g,' '))");
 const openSettings=async()=>{await evaluate("document.querySelector('.settings-button').click()");await delay(150);await evaluate("document.querySelectorAll('.settings-shell .nav-item')[1].click()");await delay(150);};
 const back=async()=>{await evaluate("document.querySelector('.settings-shell .back-to-app').click()");await delay(100);};
 await evaluate("document.querySelector('.terminal-menu-toggle').click()");await delay(150);
 results.before=await menuLabels();
 await evaluate("document.querySelector('.terminal-menu-toggle').click()");
 await openSettings();
 await evaluate("document.querySelector('[data-menu-item=codex] button').click()");await delay(100);
 await evaluate("document.querySelector('[data-menu-item=codex] button').click()");await delay(100);
 results.settingsOrder=await evaluate("Array.from(document.querySelectorAll('[data-menu-item]')).map(e=>e.dataset.menuItem)");
 await evaluate("document.querySelector('.session-menu-settings').scrollIntoView({block:'center'})");await shot('settings-order');
 await back();await evaluate("document.querySelector('.terminal-menu-toggle').click()");await delay(150);
 results.after=await menuLabels();await shot('menu-order');
 await cdp('Page.reload');await delay(2200);
 await evaluate("document.querySelector('.terminal-menu-toggle').click()");await delay(150);
 results.reloaded=await menuLabels();
 await evaluate("document.querySelector('.terminal-menu-toggle').click()");await openSettings();
 await evaluate("document.querySelector('.session-menu-settings .heading button').click()");await delay(100);
 await back();await evaluate("document.querySelector('.terminal-menu-toggle').click()");await delay(150);
 results.reset=await menuLabels();
 results.errors=errors;
 console.log(JSON.stringify(results,null,2));
 await fs.writeFile(path.join(dir,'report.json'),JSON.stringify(results,null,2));
 if(results.settingsOrder[0]!=='codex'||results.after[0]!=='Codex'||results.reloaded[0]!=='Codex')throw Error('Reorder or persistence failed');
 if(JSON.stringify(results.before)!==JSON.stringify(results.reset))throw Error('Reset failed');
 if(errors.length)throw Error('Runtime errors');
}finally{socket?.close();chrome.kill();}
