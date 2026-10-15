
const {DatabaseSync}=require('node:sqlite');
const path=require('node:path'),fs=require('node:fs');
const root=path.join(process.env.LOCALAPPDATA,'com.user.winmux','loop-routing');
const db=new DatabaseSync(path.join(root,'routing.sqlite3'));
try {
 const saved=JSON.parse(db.prepare('select data from state where id=1').get().data);
 const migrated=[];
 for(const g of saved.groups) {
  const a=g.attempts.at(-1);
  if(g.status==='stopped'||g.pendingWork||!a||a.initialPrompt!==undefined||a.continuation||a.expectedSessionId||!a.reference||fs.existsSync(a.reference.transcriptPath))continue;
  if(!/^[0-9a-f-]{36}$/i.test(g.id)||!/^[0-9a-f-]{36}$/i.test(a.id))continue;
  const dir=path.join(root,g.id,a.id),ready=path.join(dir,'startup-ready.json');
  if(!fs.existsSync(ready)||JSON.parse(fs.readFileSync(ready,'utf8')).sessionId!==a.reference.id)continue;
  const events=[];
  for(const folder of ['events','processed-events']) {
   const d=path.join(dir,folder);
   if(fs.existsSync(d))for(const name of fs.readdirSync(d).filter(n=>n.endsWith('.json')))events.push(JSON.parse(fs.readFileSync(path.join(d,name),'utf8')));
  }
  if(!events.length||events.some(e=>e.kind!=='SessionStart'||e.sessionId!==a.reference.id))continue;
  a.initialPrompt=true;
  migrated.push(g.id);
 }
 if(migrated.length)db.prepare('update state set data=? where id=1').run(JSON.stringify(saved));
 console.log(JSON.stringify({migratedUnusedGroups:migrated}));
} finally {db.close();}
