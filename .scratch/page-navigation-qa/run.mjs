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
 await shot('desktop-initial');
 const results={};
 results.initial=await evaluate("({noWorkModeTabs:!document.querySelector('.work-mode'),flowBeforeAdd:!!(document.querySelector('.open-flow').compareDocumentPosition(document.querySelector('.add-terminal'))&Node.DOCUMENT_POSITION_FOLLOWING),terminalHtml:document.querySelector('.pane .body')?.innerHTML})");
 await evaluate("window.__qaTerminal=document.querySelector('.term-host');document.querySelector('.open-flow').click()");await delay(500);await shot('desktop-flow');
 results.flowOpen=await evaluate("({visible:!!document.querySelector('.flow-page')&&getComputedStyle(document.querySelector('.flow-page')).display!=='none',terminalPreserved:!!window.__qaTerminal&&window.__qaTerminal===document.querySelector('.term-host')})");
 await evaluate("document.querySelector('.flow-page .close').click()");await delay(200);
 results.flowClosed=await evaluate("getComputedStyle(document.querySelector('.flow-page')).display==='none'");
 await evaluate("document.querySelector('.terminal-menu-toggle').click()");await delay(150);await evaluate("document.querySelector('.terminal-option').click()");
 for(let i=0;i<120;i++){if(await evaluate("!!document.querySelector('.monaco-editor')"))break;await delay(500);}
 await delay(400);await shot('desktop-editor-blank');
 results.editorBlank=await evaluate("!!document.querySelector('.monaco-editor')&&!document.querySelector('.view-lines').textContent.trim()");

 const pos=await evaluate("(()=>{const r=document.querySelector('.monaco-editor .view-lines').getBoundingClientRect();return {x:r.x+8,y:r.y+8}})()");await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...pos});await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...pos});await cdp('Input.insertText',{text:'QA note content'});await delay(300);
 await evaluate("window.__qaCancel=true;document.querySelector('.file-viewer .toolbar button').click()");await delay(300);
 results.cancel=await evaluate("({writeCount:__qaCalls.filter(c=>c.command==='write_file').length,text:document.querySelector('.view-lines').textContent})");
 await evaluate("window.__qaCancel=false;document.querySelector('.file-viewer .toolbar button').click()");await delay(400);await shot('desktop-saved');
 results.saved=await evaluate("({calls:__qaCalls.filter(c=>['write_file','plugin:dialog|save'].includes(c.command)),title:document.querySelector('.file-viewer .path').textContent,body:document.body.innerText})");
 await cdp('Emulation.setDeviceMetricsOverride',{width:800,height:700,deviceScaleFactor:1,mobile:false});await delay(200);await shot('narrow-editor');
 results.narrowEditor=await evaluate("({width:innerWidth,scrollWidth:document.documentElement.scrollWidth})");
 await evaluate("document.querySelector('.open-flow').click()");await delay(200);await shot('narrow-flow');
 results.narrowFlow=await evaluate("({width:innerWidth,scrollWidth:document.documentElement.scrollWidth})");
 results.errors=errors;await fs.writeFile(path.join(dir,'report.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results,null,2));
 await fs.writeFile(path.join(dir,'initial-errors.json'),JSON.stringify(errors,null,2));
}finally{socket?.close();chrome.kill();}


 const report=JSON.parse(await fs.readFile(path.join(dir,'report.json'),'utf8'));if(report.saved.calls.find(c=>c.command==='write_file')?.args.contents!=='QA note content'||report.saved.calls.filter(c=>c.command==='plugin:dialog|save').length!==2)throw Error('Saved content or cancel assertion failed');
