import {test,expect} from 'bun:test';
import {Database} from 'bun:sqlite';
import {readFileSync,copyFileSync,mkdirSync} from 'node:fs';
import {sqliteHost} from '../auth-policy/sqlite-host.ts';
import {admitGuest} from './admission.ts';
import lock from './artifact-lock.json';
const wasm=readFileSync('experiments/auth-policy/build/application.wasm');
const contract=readFileSync('experiments/auth-policy/build/contract.json');
const seed=JSON.parse(readFileSync('build/auth-policy/seed.json','utf8'));
const note=seed.notes[0];
mkdirSync('build/auth-policy-measure',{recursive:true});
let serial=0;
function database(){const path=`build/auth-policy-measure/trust-${++serial}.sqlite`;copyFileSync('build/auth-policy/seed.sqlite',path);return new Database(path);}
function request(user='alice',body={id:note.id}){return {method:'POST',path:'/notes/read',authorization:`Bearer ${seed.credentials[user]}`,cookie:null,body:JSON.stringify(body),configuration:seed.configuration,now:seed.now};}

test('reviewed module and contract admit; real guest checks principals and live revocation',()=>{
 const db=database(),host=sqliteHost(db),{guest}=admitGuest(wasm,contract,lock);
 try{
  expect(guest.invoke(-1,request(),host.reply)).toEqual({kind:'success',value:{status:200,body:{id:note.id,title:note.title}}});
  const denied=guest.invoke(-1,request('charlie'),host.reply);expect(denied.value.status).toBe(404);
  db.prepare('UPDATE __jadpo_auth_sessions SET revoked=1 WHERE id=?').run(seed.credentials.alice.split('.')[1]);
  expect(guest.invoke(-1,request(),host.reply).value.status).toBe(401);
 }finally{host.recover();db.close();}
});
test('modified module, replacement valid module and changed contract rejected before execution',()=>{
 const modified=Uint8Array.from(wasm);modified[modified.length-1]^=1;
 expect(()=>admitGuest(modified,contract,lock)).toThrow('unapproved application artifact');
 const emptyValidModule=Uint8Array.from([0,97,115,109,1,0,0,0]);expect(WebAssembly.validate(emptyValidModule)).toBe(true);
 expect(()=>admitGuest(emptyValidModule,contract,lock)).toThrow('unapproved application artifact');
 const changed=Buffer.from(contract.toString().replace('policy-experiment','different-audience'));
 expect(Buffer.compare(changed,contract)).not.toBe(0);expect(()=>admitGuest(wasm,changed,lock)).toThrow('unapproved application artifact');
});
test('raw host is not an authority boundary: forged guest SQL reads private data without authentication',()=>{
 const db=database(),host=sqliteHost(db);
 try{
  const r=host.reply('sql.query',{sql:'SELECT private_note FROM note WHERE id=?',params:[note.id]});
  expect(r).toEqual({kind:'success',value:[{private_note:note.private_note}]});
 }finally{host.recover();db.close();}
});
test('raw query capability can mutate in autocommit: the capability name is not an SQL sandbox',()=>{
 const db=database(),host=sqliteHost(db);
 try{
  expect(host.reply('sql.update',{sql:'UPDATE note SET title=? WHERE id=? RETURNING title',params:['Denied direct update',note.id]})).toEqual({kind:'internal'});
  const r=host.reply('sql.query',{sql:'UPDATE note SET title=? WHERE id=? RETURNING title',params:['FORGED_QUERY_WRITE',note.id]});
  expect(r).toEqual({kind:'success',value:[{title:'FORGED_QUERY_WRITE'}]});
  expect(db.inTransaction).toBe(false);host.recover();
  expect(db.prepare('SELECT title FROM note WHERE id=?').get(note.id)).toEqual({title:'FORGED_QUERY_WRITE'});
 }finally{host.recover();db.close();}
});
test('raw transaction capabilities allow policy-free commits, but cleanup rolls back unfinished writes',()=>{
 const db=database(),host=sqliteHost(db);
 try{
  expect(host.reply('transaction.begin',{}).kind).toBe('success');
  expect(host.reply('sql.update',{sql:'UPDATE note SET owner_id=? WHERE id=? RETURNING id',params:[seed.users.charlie,note.id]}).kind).toBe('success');
  expect(host.reply('transaction.commit',{}).kind).toBe('success');
  expect(db.prepare('SELECT owner_id FROM note WHERE id=?').get(note.id)).toEqual({owner_id:seed.users.charlie});
  host.reply('transaction.begin',{});host.reply('sql.update',{sql:'UPDATE note SET title=? WHERE id=? RETURNING id',params:['ROLL_BACK',note.id]});host.recover();
  expect(db.prepare('SELECT title FROM note WHERE id=?').get(note.id)).toEqual({title:note.title});
 }finally{host.recover();db.close();}
});
