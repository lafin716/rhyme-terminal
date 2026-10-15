import fs from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
const dir=path.resolve('.scratch/terminal-zoom-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
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
 await cdp('Page.navigate',{url:'http://127.0.0.1:43329/'});
 for(let i=0;i<100;i++){if(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))break;await delay(500);}
 await delay(1500);
 const assert=(value,message)=>{if(!value)throw Error(message);};
 const snapshot=()=>evaluate(`(()=>{const e=document.querySelector('.terminal-zoom-toast'),r=e?.getBoundingClientRect();return {text:e?.textContent.trim(),x:r?.x+r?.width/2,y:r?.y+r?.height/2,background:e&&getComputedStyle(e).backgroundColor,pointerEvents:e&&getComputedStyle(e).pointerEvents,scale:visualViewport.scale,writes:__qaCalls.filter(c=>c.command==='write_session').length,resizes:__qaCalls.filter(c=>c.command==='resize_session').map(c=>c.args)}})()`);
 await evaluate("document.querySelector('.xterm-helper-textarea').focus()");
 const press=async(key,code,virtualKey)=>{await cdp('Input.dispatchKeyEvent',{type:'keyDown',key,code,windowsVirtualKeyCode:virtualKey,modifiers:10});await cdp('Input.dispatchKeyEvent',{type:'keyUp',key,code,windowsVirtualKeyCode:virtualKey,modifiers:10});await delay(70);};
 const before=await snapshot();
 await press('+','Equal',187);
 const plus=await snapshot();assert(plus.text==='110%','Plus zoom failed');assert(plus.x===720&&plus.y===500,'Toast is not centered');assert(plus.scale===1,'Page zoomed');assert(plus.pointerEvents==='none','Toast blocks input');
 await shot('zoom-110');
 await press('_','Minus',189);
 const minus=await snapshot();assert(minus.text==='100%','Shift minus failed');
 const pos=await evaluate("(()=>{const r=document.querySelector('.term-host').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()");
 await cdp('Input.dispatchMouseEvent',{type:'mouseWheel',...pos,deltaX:0,deltaY:-120,modifiers:10});await delay(120);
 const wheel=await snapshot();assert(wheel.text==='110%','Wheel zoom failed');assert(wheel.writes===before.writes,'Zoom wrote to PTY');assert(wheel.resizes.length>before.resizes.length,'Zoom did not resize PTY');
 await delay(1400);assert(!(await snapshot()).text,'Toast did not hide');
 await cdp('Emulation.setDeviceMetricsOverride',{width:800,height:600,deviceScaleFactor:1,mobile:false});await delay(200);
 await press('+','NumpadAdd',107);const small=await snapshot();assert(small.text==='120%'&&small.x===400&&small.y===300,'Small window zoom or center failed');
 await shot('zoom-120-small');assert(errors.length===0,'Browser exceptions');
 const report={before,plus,minus,wheel,small,errors};await fs.writeFile(path.join(dir,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
} finally {socket?.close();chrome.kill();}