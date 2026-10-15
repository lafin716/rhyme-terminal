import http from 'node:http';
import fs from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';

const work = path.resolve('.scratch/mobile-pairing/work');
const root = path.resolve('dist-mobile');
const browser = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const profile = path.join(work, `chrome-${Date.now()}`);
await fs.mkdir(profile, { recursive: true });
const server = http.createServer(async (req, res) => {
  const name = new URL(req.url, 'http://localhost').pathname;
  const target = path.resolve(root, '.' + (name === '/' ? '/mobile.html' : name));
  if (!target.startsWith(root + path.sep)) { res.writeHead(403).end(); return; }
  try {
    const data = await fs.readFile(target);
    const ext = path.extname(target);
    res.setHeader('Content-Type', ({ '.html':'text/html; charset=utf-8', '.js':'text/javascript', '.css':'text/css' })[ext] || 'application/octet-stream');
    res.end(data);
  } catch { res.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const chrome = spawn(browser, ['--headless=new', '--disable-gpu', '--no-first-run', '--no-default-browser-check', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'], { windowsHide: true, stdio: 'ignore' });
let socket;
try {
  let debugPort;
  for (let i=0;i<100;i++) {
    try { debugPort = Number((await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]); break; } catch { await new Promise(r=>setTimeout(r,200)); }
  }
  if (!debugPort) throw new Error('Chrome debugging endpoint did not start');
  const tabs = await (await fetch(`http://127.0.0.1:${debugPort}/json/list`)).json();
  socket = new WebSocket(tabs.find(t=>t.type==='page').webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject;});
  let seq=0;
  const pending=new Map(), errors=[];
  socket.onmessage=e=>{
    const message=JSON.parse(e.data);
    if(message.id) { const p=pending.get(message.id);pending.delete(message.id);if(p)clearTimeout(p.timer);message.error?p?.reject(message.error):p?.resolve(message.result); }
    if(message.result?.exceptionDetails)errors.push(JSON.stringify(message.result.exceptionDetails)); if(message.method==='Runtime.exceptionThrown') errors.push(message.params.exceptionDetails.text);
  };
  const cdp=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>{pending.delete(id);reject(new Error("CDP timeout: "+method));},15000);pending.set(id,{resolve,reject,timer});socket.send(JSON.stringify({id,method,params}));});
  await cdp('Page.enable'); await cdp('Runtime.enable');
  const desktop = process.argv.includes('--desktop');
  await cdp('Emulation.setDeviceMetricsOverride',{width:desktop?1100:390,height:desktop?1200:844,deviceScaleFactor:1,mobile:!desktop});
  const keyboard = process.argv.includes('--keyboard');
  const mock = process.argv.includes('--mock') || keyboard;
  if (mock) await cdp('Page.addScriptToEvaluateOnNewDocument',{source: `
    window.__qaInputs = []; window.__qaMessages = [];
    const info={id:'11111111-1111-4111-8111-111111111111',name:'w1.mobile-test',shell:'powershell.exe',cwd:null,cols:100,rows:24};
    window.WebSocket=class {
      readyState=0;
      constructor(){setTimeout(()=>{this.readyState=1;this.onopen?.({});},10);}
      emit(data){setTimeout(()=>this.onmessage?.({data:JSON.stringify(data)}),10);}
      send(raw){const m=JSON.parse(raw);window.__qaMessages.push(m); if(m.type==='pair'){this.emit({type:'pending',verification:'123456'});setTimeout(()=>this.emit({type:'authenticated',token:'qa-device-token',deviceId:'qa'}),100);}
      else if(m.type==='auth')this.emit({type:'authenticated',token:'qa-device-token',deviceId:'qa'});
      else if(m.type==='list')this.emit({type:'result',requestId:m.requestId,data:[info]});
      else if(m.type==='attach'){this.emit({type:'result',requestId:m.requestId,data:{info,scrollback:btoa('PS C:\\work> echo mobile pairing\\r\\nmobile pairing ready\\r\\n')}});setTimeout(()=>this.emit({type:'event',data:{event:'pty_output',id:info.id,data:btoa('live output verified\\r\\n')}}),100);}
      else {if(m.type==='input')window.__qaInputs.push(m.data);this.emit({type:'result',requestId:m.requestId,data:null});}}
      close(){this.readyState=3;setTimeout(()=>this.onclose?.({}),0);}
    };
  `});
  await cdp('Page.navigate',{url:desktop?'http://127.0.0.1:43299/.scratch/mobile-pairing/work/desktop-qa.html':origin+(mock?'#invite=qa-invite':'')});
  await new Promise(r=>setTimeout(r,2000));
  if(desktop) {
    for(let i=0;i<100;i++){const ready=await cdp('Runtime.evaluate',{expression:'!!document.querySelector('+JSON.stringify('button.primary:not(:disabled)')+')',returnByValue:true});if(ready.result.value)break;await new Promise(r=>setTimeout(r,200));}
    await cdp('Runtime.evaluate',{expression:`document.querySelector('button.primary').click();`});
    await new Promise(r=>setTimeout(r,500));
    const screenshot=await cdp('Page.captureScreenshot',{format:'png'});
    await fs.writeFile(path.join(work,'desktop-settings.png'),Buffer.from(screenshot.data,'base64'));
    await cdp('Runtime.evaluate',{expression:`Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()==='승인').click();`});
    await new Promise(r=>setTimeout(r,300));
    await cdp('Runtime.evaluate',{expression:`document.querySelector('.device button.danger').click();`});
    await new Promise(r=>setTimeout(r,300));
    await cdp('Runtime.evaluate',{expression:`document.querySelector('.actions button.danger').click();`});
    await new Promise(r=>setTimeout(r,300));
  }
  if(mock) {
    await cdp('Runtime.evaluate',{expression:`document.querySelector('select').value='11111111-1111-4111-8111-111111111111';document.querySelector('select').dispatchEvent(new Event('change',{bubbles:true}));`});
    await new Promise(r=>setTimeout(r,1000));
    if(keyboard) {
      const position=await cdp('Runtime.evaluate',{expression:`(()=>{const r=document.querySelector('.xterm-screen').getBoundingClientRect();return {x:r.x+40,y:r.y+25};})()`,returnByValue:true});
      if(process.argv.includes('--touch')) {
        await cdp('Emulation.setTouchEmulationEnabled',{enabled:true,maxTouchPoints:1});
        await cdp('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[position.result.value]});
        await cdp('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
        await new Promise(r=>setTimeout(r,400));
      } else {
      await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',clickCount:1,...position.result.value});
      await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',clickCount:1,...position.result.value});
      }
      for(const key of ['a','b']) {
        await cdp('Input.dispatchKeyEvent',{type:'keyDown',key,code:'Key'+key.toUpperCase(),text:key});
        await cdp('Input.dispatchKeyEvent',{type:'keyUp',key,code:'Key'+key.toUpperCase()});
      }
      await cdp('Input.dispatchKeyEvent',{type:'keyDown',key:'Enter',code:'Enter',text:'\r',windowsVirtualKeyCode:13});
      await cdp('Input.dispatchKeyEvent',{type:'keyUp',key:'Enter',code:'Enter',windowsVirtualKeyCode:13});
    } else {
    await cdp('Runtime.evaluate',{expression:`const el=document.querySelector('.input-panel textarea');el.value='echo test';el.dispatchEvent(new Event('input',{bubbles:true}));`});
    await new Promise(r=>setTimeout(r,100));
    await cdp('Runtime.evaluate',{expression:`document.querySelector('button.send').click();`});
    }
    await new Promise(r=>setTimeout(r,200));
  }
  const result=await cdp('Runtime.evaluate',{expression:'JSON.stringify({text:document.body.innerText,scrollWidth:document.documentElement.scrollWidth,width:innerWidth,title:document.title,inputs:window.__qaInputs ?? [],messages:window.__qaMessages ?? [],calls:window.__qaCalls ?? [],terminal:!!document.querySelector(".xterm-screen")})',returnByValue:true});
  const screenshot=await cdp('Page.captureScreenshot',{format:'png'});
  if(!desktop) await fs.writeFile(path.join(work,mock?'mobile-connected.png':'mobile-browser.png'),Buffer.from(screenshot.data,'base64'));
  const report={...JSON.parse(result.result.value),errors};
  await fs.writeFile(path.join(work,'browser-result.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify(report));
  if(errors.length || !report.text.trim() || (mock && (!report.terminal || (keyboard ? report.inputs.map(s=>Buffer.from(s,'base64').toString('utf8')).join('') !== 'ab\r' : report.inputs.length !== 1) || report.scrollWidth > report.width))) process.exitCode=1;
  if(desktop && !['mobile_pairing_start','mobile_pairing_approve','mobile_pairing_revoke','mobile_pairing_stop'].every(command=>report.calls.some(call=>call.command===command))) process.exitCode=1;
} finally { socket?.close(); chrome.kill(); server.close(); }





