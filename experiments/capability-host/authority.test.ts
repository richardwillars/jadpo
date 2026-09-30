import {test,expect} from 'bun:test';
import {Database} from 'bun:sqlite';
import {readFileSync,mkdtempSync,copyFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {Guest} from './wasm-driver.ts';
import {AuthorityHost,authorityGuest,type ApplicationGuest} from './authority-host.ts';
const base=import.meta.dir+'/build/',lock=JSON.parse(readFileSync(base+'trust.json','utf8'));
const authority=readFileSync(base+'authority.wasm'),application=readFileSync(base+'application.wasm');
const seed=JSON.parse(readFileSync('build/capability-host/seed.json','utf8'));
const [a,b,c]=seed.notes;
const frame=(user='alice',path='/notes/read',body:any={id:a.id})=>({method:'POST',path,authorization:user?`Bearer ${seed.credentials[user]}`:null,cookie:null,body:JSON.stringify(body),now:seed.now,configuration:seed.configuration});
const pair=()=>frame('alice','/notes/pair',{first:a.id,second:b.id,title:'First changed',second_title:'Second changed'});
function fixture(wrap:(g:Guest,db:Database)=>ApplicationGuest=g=>g){
 const dir=mkdtempSync(join(tmpdir(),'jadpo-capability-'));copyFileSync('build/capability-host/seed.sqlite',dir+'/state.sqlite');
 const db=new Database(dir+'/state.sqlite'),module=new WebAssembly.Module(application),host=new AuthorityHost(authorityGuest(authority,lock.authority),()=>wrap(new Guest(module),db),db);
 return {db,host,snapshot:()=>db.prepare('SELECT * FROM note ORDER BY id').all(),close(){db.close();rmSync(dir,{recursive:true,force:true});}};
}
const fake=(g:Guest,change:(s:any,phase:string)=>any):ApplicationGuest=>({start:(op,input)=>change(g.start(op,input),'start'),resume:(p,v)=>change(g.resume(p,v),'resume'),cancel:()=>g.cancel()});

test('application sees only input and permitted row fields; no credential, key, clock or SQL',()=>{
 const observations:any[]=[];
 const f=fixture(g=>({start:(op,input)=>{observations.push({op,input});return g.start(op,input)},resume:(p,v)=>{observations.push(v);return g.resume(p,v)},cancel:()=>g.cancel()}));
 try{
  expect(f.host.invoke(frame('bob')).value).toEqual({status:200,body:{id:a.id,title:a.title}});
  const text=JSON.stringify(observations);
  for(const value of [...Object.values(seed.credentials),...Object.values(seed.configuration),a.private_note])expect(text).not.toContain(value as string);
  expect(text).not.toContain('configuration');expect(text).not.toContain('SELECT');expect(text).not.toContain('now');
  expect(observations[1].value.private_note).toBe(null);
  expect(f.host.invoke(frame('alice','/notes/private')).value.body.private_note).toBe(a.private_note);
 }finally{f.close();}
});
test('revocation denies before invoking application; identity and row policy stay fresh',()=>{
 let starts=0;const f=fixture(g=>({start:(op,input)=>{starts++;return g.start(op,input)},resume:(p,v)=>g.resume(p,v),cancel:()=>g.cancel()}));
 try{
  expect(f.host.invoke(frame()).value.status).toBe(200);
  f.db.prepare('UPDATE __jadpo_auth_sessions SET revoked=1 WHERE id=?').run(seed.credentials.alice.split('.')[1]);
  expect(f.host.invoke(frame()).value.status).toBe(401);expect(starts).toBe(1);
  expect(f.host.invoke(frame('charlie')).value.status).toBe(404);
  f.db.prepare('UPDATE note SET owner_id=? WHERE id=?').run(seed.users.charlie,a.id);
  expect(f.host.invoke(frame('charlie','/notes/private')).value.body.private_note).toBe(a.private_note);
  f.db.prepare('UPDATE user SET enabled=0 WHERE id=?').run(seed.users.charlie);
  expect(f.host.invoke(frame('charlie')).value.status).toBe(422);
 }finally{f.close();}
});
test('ownership is checked in SQL after guest start, not cached with authentication',()=>{
 const f=fixture((g,db)=>({start:(op,input)=>{db.prepare('UPDATE note SET owner_id=?,editor_id=? WHERE id=?').run(seed.users.charlie,seed.users.charlie,a.id);return g.start(op,input)},resume:(p,v)=>g.resume(p,v),cancel:()=>g.cancel()}));
 try{expect(f.host.invoke(frame()).value.status).toBe(404);}finally{f.close();}
});
const forgeries:Record<string,(p:any)=>any>={
 raw_sql:p=>({...p,capability:'sql.query',args:{sql:'SELECT private_note FROM note',params:[]}}),
 commit:p=>({...p,capability:'transaction.commit',args:null}),
 wrong_plan:p=>({...p,args:{...p.args,plan:43}}),
 wrong_row:p=>({...p,args:{...p.args,key:c.id}}),
 forged_principal:p=>({...p,args:{...p.args,principal:{user_id:seed.users.charlie}}}),
 SQL_injection_key:p=>({...p,args:{...p.args,key:"' OR 1=1 --"}}),
 stale_operation:p=>({...p,operationId:0}),
 noninteger_request:p=>({...p,requestId:1.5}),
 extra_frame_field:p=>({...p,trusted:true}),
 premature_completion:()=>({kind:'success',value:{id:a.id,title:a.title}}),
};
for(const [name,forge] of Object.entries(forgeries))test('forged first effect rejected: '+name,()=>{
 const f=fixture(g=>fake(g,(step,phase)=>phase==='start'?forge(step):step));
 try{const before=f.snapshot();expect(f.host.invoke(frame()).value.status).toBe(500);expect(f.snapshot()).toEqual(before);expect(f.db.inTransaction).toBe(false);}finally{f.close();}
});
for(const name of ['extra_field','different_value','commit_after_first','replayed_first','cross_request','premature_completion','wrong_final','oversized_final','trap_and_cancel_trap'])test('untrusted write protocol rolls back: '+name,()=>{
 let first:any;const f=fixture(g=>({
  start:(op,input)=>{const p=g.start(op,input);first=structuredClone(p);if(name==='extra_field')p.args.changes.owner_id=seed.users.charlie;if(name==='different_value')p.args.changes.title='Attacker title';return p;},
  resume:(p,v)=>{
   const next=g.resume(p,v);
   if(name==='trap_and_cancel_trap')throw Error('simulated guest trap');
   if(next.kind==='pending'){
    if(name==='commit_after_first')return {...next,capability:'transaction.commit',args:null};
    if(name==='replayed_first')return first;
    if(name==='cross_request')return {...next,requestId:next.requestId+1};
    if(name==='premature_completion')return {kind:'success',value:{id:b.id,title:'Second changed'}};
   }else{
    if(name==='wrong_final')return {kind:'success',value:{id:b.id,title:'Forged completion'}};
    if(name==='oversized_final')return {kind:'success',value:{id:b.id,title:'x'.repeat(70000)}};
   }
   return next;
  },
  cancel:()=>{g.cancel();if(name==='trap_and_cancel_trap')throw Error('cancel trap');},
 }));
 try{const before=f.snapshot();expect(f.host.invoke(pair()).value.status).toBe(500);expect(f.snapshot()).toEqual(before);expect(f.db.inTransaction).toBe(false);}finally{f.close();}
});
test('captured capability from prior request is rejected on instance reuse',()=>{
 let old:any,replay=false;const f=fixture(g=>fake(g,(p,phase)=>{if(phase==='start'){if(replay)return old;old=structuredClone(p);}return p}));
 try{expect(f.host.invoke(frame()).value.status).toBe(200);replay=true;expect(f.host.invoke(frame('charlie')).value.status).toBe(500);}finally{f.close();}
});
test('different and alternating application instances cannot select authority',()=>{
 const one=fixture(),two=fixture();try{
  for(let i=0;i<20;i++){
   expect(two.host.invoke(frame(i%2?'alice':'bob','/notes/private')).value.status).toBe(i%2?200:404);
   expect(one.host.invoke(frame(i%2?'bob':'alice','/notes/private')).value.status).toBe(i%2?404:200);
  }
 }finally{one.close();two.close();}
});
test('modified trusted authority bytes are rejected',()=>{
 const changed=Uint8Array.from(authority);changed[changed.length-1]^=1;
 expect(()=>authorityGuest(changed,lock.authority)).toThrow('unapproved authority artifact');
});
for(const mode of [1,2,3,4])test('actual malicious WASM rejected, attack mode '+mode,()=>{
 const f=fixture(()=>{const evil=new Guest(readFileSync(base+'attack.wasm'));evil.exports.attack_mode(mode);return evil});
 try{const before=f.snapshot();expect(f.host.invoke(frame('bob')).value.status).toBe(500);expect(f.snapshot()).toEqual(before);expect(f.db.inTransaction).toBe(false);}finally{f.close();}
});

test('each invocation receives fresh application memory and a new authority scope',()=>{
 const memories:any[]=[],scopes:number[]=[];
 const f=fixture(g=>{
  const memory=g.exports.memory.buffer;expect(memories).not.toContain(memory);
  expect(Buffer.from(memory).includes(Buffer.from(a.private_note))).toBe(false);memories.push(memory);
  return {start:(op,input)=>{scopes.push(input.scope);return g.start(op,input)},resume:(p,v)=>g.resume(p,v),cancel:()=>g.cancel()};
 });
 try{
  expect(f.host.invoke(frame('alice','/notes/private')).value.status).toBe(200);
  expect(f.host.invoke(frame('bob')).value.status).toBe(200);
  expect(memories.length).toBe(2);expect(scopes[1]).toBeGreaterThan(scopes[0]);
 }finally{f.close();}
});
test('invalid high request ID cannot poison the next fresh invocation',()=>{
 let attack=true;
 const f=fixture(g=>fake(g,(p,phase)=>phase==='start'&&attack?{...p,requestId:4294967295}:p));
 try{expect(f.host.invoke(frame()).value.status).toBe(500);attack=false;expect(f.host.invoke(frame()).value.status).toBe(200);}finally{f.close();}
});
test('private bridge framing preserves frozen raw-WASM row/completion byte budgets',()=>{
 const raw=new Guest(readFileSync('experiments/auth-policy/build/application.wasm'));
 const f=fixture();const host=require('./sqlite-host.ts').sqliteHost(f.db);
 try{
  for(const size of [64000,65000,65100,65200,65300,65400,65500,65535,65536,70000]){
   f.db.prepare('UPDATE note SET private_note=? WHERE id=?').run('x'.repeat(size),a.id);
   for(const path of ['/notes/read','/notes/private','/notes/rename']){
    const input=frame('alice',path,path.endsWith('rename')?{id:a.id,title:'Budget rename'}:{id:a.id});
    const expected=raw.invoke(-1,input,host.reply);host.recover();
    expect(f.host.invoke(input)).toEqual(expected);expect(f.db.inTransaction).toBe(false);
   }
  }
 }finally{f.close();}
});
