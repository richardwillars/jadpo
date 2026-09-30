import {test,expect} from 'bun:test';
import {Database} from 'bun:sqlite';
import {readFileSync,mkdtempSync,copyFileSync,rmSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {Guest} from './wasm-driver.ts';
import {sqliteHost} from './sqlite-host.ts';
const seed=JSON.parse(readFileSync(resolve('build/auth-policy/seed.json'),'utf8'));
const bytes=readFileSync(import.meta.dir+'/build/application.wasm');
function fixture(){const dir=mkdtempSync(join(tmpdir(),'jadpo-auth-wasm-'));copyFileSync(resolve('build/auth-policy/seed.sqlite'),dir+'/state.sqlite');const db=new Database(dir+'/state.sqlite');const host=sqliteHost(db);return {db,host,close(){host.recover();db.close();rmSync(dir,{recursive:true,force:true});}};}
const input=(user:string,path='/notes/private',body:any={id:seed.notes[0].id})=>({method:'POST',path,authorization:`Bearer ${seed.credentials[user]}`,cookie:null,body:JSON.stringify(body),now:seed.now,configuration:seed.configuration});
test('actual WASM verifies generated opaque credentials and isolates reversed suspended instances',()=>{
 const f=fixture();try{
  const alice=new Guest(bytes),bob=new Guest(bytes),a=alice.start(-1,input('alice')),b=bob.start(-1,input('bob'));
  expect(bob.drive(b,f.host.reply).value.status).toBe(404);
  expect(alice.drive(a,f.host.reply).value.body.private_note).toBe('ALICE_PRIVATE_1');
  for(let i=0;i<40;i++)expect(alice.invoke(-1,input(i%2?'alice':'bob'),f.host.reply).value.status).toBe(i%2?200:404);
 }finally{f.close();}
});
test('actual WASM cancellation after first SQLite write rolls back before reuse',()=>{
 const f=fixture();try{
  const guest=new Guest(bytes);let step=guest.start(-1,input('alice','/notes/pair',{first:seed.notes[0].id,second:seed.notes[1].id,title:'First changed',second_title:'Second changed'}));
  while(step.kind==='pending'){
   const updated=step.capability==='sql.update';step=guest.resume(step,f.host.reply(step.capability,step.args));
   if(updated)break;
  }
  expect((f.db.prepare('SELECT title FROM note WHERE id=?').get(seed.notes[0].id) as any).title).toBe('First changed');
  expect(step.capability).toBe('sql.update');guest.cancel();f.host.recover();
  expect((f.db.prepare('SELECT title FROM note WHERE id=?').get(seed.notes[0].id) as any).title).toBe('Alice first');
  expect(guest.invoke(-1,input('bob'),f.host.reply).value.status).toBe(404);
 }finally{f.close();}
});
test('revocation between the two session reads and forged host results fail closed',()=>{
 const f=fixture();try{
  const guest=new Guest(bytes);let step=guest.start(-1,input('alice'));step=guest.resume(step,f.host.reply(step.capability,step.args));
  f.db.exec('UPDATE __jadpo_auth_sessions SET revoked=1');expect(guest.drive(step,f.host.reply).value.body.error.code).toBe('invalid_credentials');
  step=guest.start(-1,input('alice'));expect(guest.resume(step,{kind:'success',value:{principal:{user_id:seed.users.alice}}}).value.status).toBe(503);
 }finally{f.close();}
});
test('raw callable ABI cannot bypass authentication; wrong request handles fail',()=>{
 const guest=new Guest(bytes);expect(guest.start(45,{id:seed.notes[0].id}).kind).toBe('invalid');
 const pending=guest.start(-1,input('alice'));expect(guest.resume({...pending,requestId:pending.requestId+1},{kind:'success',value:[]}).kind).toBe('internal');
});
