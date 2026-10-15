import fs from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
const dir=path.resolve('.scratch/explorer-scroll-qa'),profile=path.join(dir,`chrome-${Date.now()}`);
await fs.mkdir(profile,{recursive:true});
const chrome=spawn('C:/Program Files/Google/Chrome/Application/chrome.exe',['--headless=new','--disable-gpu','--no-first-run','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'],{windowsHide:true,stdio:'ignore'});
let socket;
const delay=ms=>new Promise(r=>setTimeout(r,ms));
try {
 let port;for(let i=0;i<100;i++){try{port=Number((await fs.readFile(path.join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0]);break;}catch{await delay(200);}}
 if(!port)throw Error('No Chrome endpoint');
 const tabs=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();socket=new WebSocket(tabs.find(t=>t.type==='page').webSocketDebuggerUrl);await new Promise((r,j)=>{socket.onopen=r;socket.onerror=j;});
 let seq=0;const pending=new Map();
 socket.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result);}};
 const cdp=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>reject(Error('CDP timeout: '+method)),10000);pending.set(id,{resolve:v=>{clearTimeout(timer);resolve(v)},reject});socket.send(JSON.stringify({id,method,params}));});
 const evaluate=async expression=>{const r=await cdp('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
 const source=await fs.readFile('src/components/ExplorerPanel.vue','utf8');
 const css=source.match(/<style scoped>([\s\S]*?)<\/style>/)[1];
 await cdp('Page.enable');
 const {frameTree}=await cdp('Page.getFrameTree');
 const report=[];
 for(const height of [600,240]) {
  await cdp('Page.setDocumentContent',{frameId:frameTree.frame.id,html:`<!doctype html><style>*{box-sizing:border-box}body{margin:0}.region{height:${height}px;width:280px;padding-top:36px;overflow:hidden}${css}</style><div class="region"><section class="explorer"><div class="body"><nav class="strip"><button class="tool">Files</button></nav><div class="head"><span class="title">PROJECT</span><button class="sync">S</button></div><div class="tree">${Array.from({length:200},(_,i)=>`<div class="row"><span class="entry-name">file-${i}.txt</span></div>`).join('')}</div></div></section></div>`});
  const before=await evaluate(`(()=>{const e=document.querySelector('.tree'),r=e.getBoundingClientRect();return {client:e.clientHeight,scroll:e.scrollHeight,top:document.querySelector('.head').getBoundingClientRect().top,x:r.x+30,y:r.y+30}})()`);
  await cdp('Input.dispatchMouseEvent',{type:'mouseWheel',x:before.x,y:before.y,deltaX:0,deltaY:400});await delay(200);
  const after=await evaluate(`(()=>{const e=document.querySelector('.tree');const wheel=e.scrollTop;e.scrollTop=e.scrollHeight;return {wheel,end:e.scrollTop,lastBottom:e.lastElementChild.getBoundingClientRect().bottom,treeBottom:e.getBoundingClientRect().bottom,headTop:document.querySelector('.head').getBoundingClientRect().top,stripHeight:document.querySelector('.strip').getBoundingClientRect().height}})()`);
  report.push({height,before,after});
 }
 console.log(JSON.stringify(report,null,2));
 if(report.some(({before,after})=>before.scroll<=before.client||after.wheel<=0||after.lastBottom>after.treeBottom+1||before.top!==after.headTop||after.stripHeight<40))throw Error('Explorer scroll regression');
 console.log('PASS: wheel scroll, last file reachable, fixed headers at both heights');
} finally {socket?.close();chrome.kill();}

