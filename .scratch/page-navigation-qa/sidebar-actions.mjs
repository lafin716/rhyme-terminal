import fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';
import {spawn} from 'node:child_process';
const dir=path.resolve('.scratch/page-navigation-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
await fs.mkdir(profile,{recursive:true});
const chrome=spawn('C:/Program Files/Google/Chrome/Application/chrome.exe',['--headless=new','--disable-gpu','--no-first-run','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'],{windowsHide:true,stdio:'ignore'});
const root = path.resolve('dist');
const server = http.createServer(async (req,res) => {
 const file=path.resolve(root,'.'+(new URL(req.url,'http://localhost').pathname==='/'?'/index.html':new URL(req.url,'http://localhost').pathname));
 if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}
 try { const data=await fs.readFile(file); res.setHeader('Content-Type',file.endsWith('.js')?'application/javascript':file.endsWith('.css')?'text/css':file.endsWith('.html')?'text/html':'application/octet-stream');res.end(data); } catch {res.writeHead(404).end();}
});
await new Promise(r=>server.listen(43309,'127.0.0.1',r));
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
 localStorage.setItem('winmux:workspaces:v1',JSON.stringify({version:1,store:{activeWorkspaceId:'other-project',workspaces:[{id:'other-project',name:'Other Project',index:2,nextSessionSeq:1,settings:{defaultCwd:'C:/other',terminal:null},layout:{kind:'leaf',id:'other-leaf',tabs:[],activeTabId:null},terminalSnapshots:{}},{id:'qa-project',name:'QA Project',index:1,nextSessionSeq:2,settings:{defaultCwd:'C:\\\\qa-project',terminal:null},layout:{kind:'leaf',id:'qa-leaf',tabs:['qa-session'],activeTabId:'qa-session'},terminalSnapshots:{}}]}}));
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(command,args={})=>{window.__qaCalls.push({command,args});if(command==='list_sessions')return [session];if(command==='create_session')return session;if(command==='attach_session')return btoa('PS C:\\\\qa-project> ');if(command==='flow_request')return {workers:[],flows:[],tasks:[],runs:[]};if(command==='read_directory')return {path:args.path,entries:[]};if(command==='plugin:dialog|save')return window.__qaCancel?null:'C:\\\\qa-project\\\\qa-note.txt';if(command==='plugin:event|listen')return ++callback;if(command.includes('is_'))return false;if(command==='list_files')return {root:args.root,files:[]};return null;}};
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 `});
 await cdp('Page.navigate',{url:'http://127.0.0.1:43309/'});
 for(let i=0;i<100;i++){if(await evaluate("!!document.querySelector('.terminal-menu-toggle')"))break;await delay(500);}
 await delay(1500);

 const check=async(expression,message)=>{if(!await evaluate(expression))throw Error(message);};
 const rightClick=async()=>{await evaluate("document.querySelector('.sidebar .session').dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,cancelable:true,clientX:100,clientY:180}))");await delay(150);};
 await rightClick();
 await check("document.querySelectorAll('.sidebar .menu [role=menuitem]').length===2",'Session menu missing');
 await evaluate("document.querySelector('.sidebar .menu-item').click()");await delay(150);
 await check("document.activeElement.matches('.session-rename-input')",'Rename focus missing');
 await evaluate("(()=>{const e=document.querySelector('.session-rename-input');e.value='renamed';e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));})()");await delay(250);
 await check("window.__qaCalls.some(c=>c.command==='rename_session'&&c.args.id==='qa-session'&&c.args.name==='w1.renamed')",'Wrong rename target');
 await check("document.querySelector('.sidebar .s-name').textContent==='renamed'",'Renamed label missing');
 await rightClick();await evaluate("document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))");await check("!document.querySelector('.sidebar .menu')",'Escape failed');
 await rightClick();await evaluate("document.querySelectorAll('.sidebar .menu-item')[1].click()");await delay(150);
 await evaluate("document.querySelector('.modal .btn:not(.primary), .overlay .btn:not(.primary)').click()");await delay(150);
 await check("!window.__qaCalls.some(c=>c.command==='kill_session')",'Cancel killed session');
 for(const width of [1440,800]){
  await cdp('Emulation.setDeviceMetricsOverride',{width,height:800,deviceScaleFactor:1,mobile:false});
  for(const kind of ['flow','settings']){
   await evaluate(kind==='flow'?"document.querySelector('.open-flow').click()":"document.querySelector('.settings-button').click()");await delay(250);
   await check("!!document.querySelector('.app-icon img')&&!document.querySelector('.shell-menu .corner-toggle')&&!document.querySelector('.window-controls .panel-toggle')",'Page actions/logo mismatch');
   await check("document.querySelectorAll('.window-controls button').length===3",'Window controls missing');
   if(kind==='settings')await check("[...document.querySelectorAll('.nav-label')].some(e=>['Providers','프로바이더'].includes(e.textContent))",'Provider label missing');
   await shot(`updated-${width}-${kind}`);
   await evaluate(kind==='flow'?"document.querySelector('.flow-page .back-to-app').click()":"document.querySelector('.settings-shell .back-to-app').click()");await delay(150);
   await check("document.querySelectorAll('.shell-menu .corner-toggle').length===2&&!!document.querySelector('.window-controls .panel-toggle')",'Terminal controls missing');
  }
 }
 await rightClick();await evaluate("document.querySelectorAll('.sidebar .menu-item')[1].click()");await delay(150);
 await evaluate("document.querySelector('.btn.primary').click()");await delay(250);
 await check("window.__qaCalls.filter(c=>c.command==='kill_session'&&c.args.id==='qa-session').length===1&&!document.querySelector('.sidebar .session')",'Delete failed');
 if(errors.length)throw Error(JSON.stringify(errors));
 console.log('PASS: session rename/delete/cancel/Escape; page logo/actions/provider label; terminal controls restored at 1440/800px');
}finally{socket?.close();chrome.kill();server.close();}

