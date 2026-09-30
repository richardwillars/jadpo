import {test,expect} from 'bun:test';
import {Database} from 'bun:sqlite';
import {readFileSync,mkdtempSync,copyFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {resolve} from 'node:path';
import {Guest} from './monitor-driver.ts';
import {MonitorHost} from './monitor-host.ts';

const base=import.meta.dir,monitor=readFileSync(base+'/monitor.wasm'),application=readFileSync(base+'/application.wasm'),attack=readFileSync(resolve(base,'../capability-host/build/attack.wasm'));
const seed=JSON.parse(readFileSync('build/capability-host/seed.json','utf8'));
const [a,b,c]=seed.notes;
const frame=(user='alice',path='/notes/read',body:any={id:a.id})=>({method:'POST',path,authorization:user?`Bearer ${seed.credentials[user]}`:null,cookie:null,body:JSON.stringify(body),now:seed.now,configuration:seed.configuration});
function fixture(factory=()=>new Guest(application)){
 const dir=mkdtempSync(join(tmpdir(),'jadpo-monitor-'));copyFileSync('build/capability-host/seed.sqlite',dir+'/state.sqlite');
 const db=new Database(dir+'/state.sqlite'),host=new MonitorHost(new Guest(monitor),factory,db);
 return {db,host,snapshot:()=>db.prepare('SELECT * FROM note ORDER BY id').all(),close(){db.close();rmSync(dir,{recursive:true,force:true});}};
}
test('single-pass monitor preserves public, private and transaction behavior',()=>{
 const f=fixture();try{
  expect(f.host.invoke(frame('alice')).value).toEqual({status:200,body:{id:a.id,title:a.title}});
  expect(f.host.invoke(frame('bob')).value).toEqual({status:200,body:{id:a.id,title:a.title}});
  expect(f.host.invoke(frame('alice','/notes/private')).value.body.private_note).toBe(a.private_note);
  expect(f.host.invoke(frame('bob','/notes/private')).value.status).toBe(404);
  expect(f.host.invoke(frame('alice','/notes/rename',{id:a.id,title:'Monitor title'})).value.body.title).toBe('Monitor title');
  const before=f.snapshot();expect(f.host.invoke(frame('alice','/notes/pair',{first:a.id,second:c.id,title:'First',second_title:'Second'})).value.status).toBe(404);expect(f.snapshot()).toEqual(before);
 }finally{f.close();}
});
test('authentication and live policy run before and during the guest',()=>{
 let starts=0;const f=fixture(()=>{starts++;return new Guest(application);});try{
  expect(f.host.invoke(frame()).value.status).toBe(200);
  f.db.prepare('UPDATE __jadpo_auth_sessions SET revoked=1 WHERE id=?').run(seed.credentials.alice.split('.')[1]);
  expect(f.host.invoke(frame()).value.status).toBe(401);expect(starts).toBe(1);
  expect(f.host.invoke(frame('charlie')).value.status).toBe(404);
  f.db.prepare('UPDATE note SET owner_id=? WHERE id=?').run(seed.users.charlie,a.id);
  expect(f.host.invoke(frame('charlie','/notes/private')).value.body.private_note).toBe(a.private_note);
 }finally{f.close();}
});
for(const mode of [1,2,3,4])test('hostile guest cannot select monitor authority '+mode,()=>{
 const evil=new Guest(attack);evil.exports.attack_mode(mode);const f=fixture(()=>evil);try{const before=f.snapshot();expect(f.host.invoke(frame('bob')).value.status).toBe(500);expect(f.snapshot()).toEqual(before);expect(f.db.inTransaction).toBe(false);}finally{f.close();}
});
test('fresh guest instances do not retain private rows',()=>{
 const memories:ArrayBuffer[]=[];const f=fixture(()=>{const guest=new Guest(application);memories.push(guest.exports.memory.buffer);return guest;});try{expect(f.host.invoke(frame('alice','/notes/private')).value.status).toBe(200);expect(f.host.invoke(frame('bob')).value.status).toBe(200);expect(memories.length).toBe(2);expect(memories[0]).not.toBe(memories[1]);}finally{f.close();}
});
test('completion must match the row returned by the trusted policy query',()=>{
 const evil=new Guest(application);
 evil.start=()=>({kind:'pending',requestId:1,operationId:1,capability:'entity.update',args:{plan:45,key:a.id,changes:{title:'Forged title'}}});
 evil.resume=()=>({kind:'success',value:{id:a.id,title:'Unrelated title'}});
 const f=fixture(()=>evil);try{const before=f.snapshot();expect(f.host.invoke(frame('alice','/notes/rename',{id:a.id,title:'Forged title'})).value.status).toBe(500);expect(f.snapshot()).toEqual(before);expect(f.db.inTransaction).toBe(false);}finally{f.close();}
});
