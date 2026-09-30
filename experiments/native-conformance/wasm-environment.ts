// Local Bun-hosted WASM conformance only. This is not a workerd measurement.
import {Database} from 'bun:sqlite';
import {readFileSync} from 'node:fs';
import {createStorage} from '../wasm-exp1/boundary-http/adapter.ts';
import {bunSqlite} from '../wasm-exp1/optimization/cached-sqlite.ts';
import {loadGuest} from './wasm-driver.ts';
export {originalRows,spec} from './bun-environment.ts';
import {originalRows,spec} from './bun-environment.ts';
export async function environment(_target:string,path:string){
 const db=new Database(path,{strict:true});db.exec('PRAGMA foreign_keys=ON');
 const program=JSON.parse(readFileSync(import.meta.dir+'/build/program.json','utf8'));
 const manifest=JSON.parse(readFileSync(import.meta.dir+'/build/manifest.json','utf8'));
 const adapter=bunSqlite(db);
 // Existing unqualified fallback-conflict contract includes every normalized
 // SQLite constraint class, not just primary-key/unique constraints.
 adapter.isUniqueConflict=(error:any)=>String(error?.code??'').startsWith('SQLITE_CONSTRAINT');
 const storage=createStorage(program,adapter),guest=loadGuest();storage.setup();
 const reset=(note:boolean|string=false)=>{const rows=structuredClone(originalRows);if(note!==false)rows[0].note=typeof note==='string'?note:'x'.repeat(16384);storage.resetFixture({Item:rows});return rows;};
 const snapshot=()=>storage.snapshot().Item;
 const call=async(operation:string,input:any,principalName='owner')=>{
  const entry=manifest.entries.find((e:any)=>e.name===operation),principal=spec.seeds.principals[principalName];if(!entry||!principal)return {kind:'invalid'};
  let first:any;try{first=guest.start(entry.semanticId,input);}catch{return {kind:'invalid'};}
  if(first.kind!=='pending')return first;
  const execute=()=>guest.drive(first,(cap,args)=>{const p={entity:principal.entity,values:{id:principal.id}};if(cap==='storage.read')return storage.read(args,p);if(cap==='storage.update')return storage.update(args,p);throw Error('capability');});
  try{return entry.atomic?storage.runAtomic(execute):execute();}catch{guest.cancel();return {kind:'internal'};}
 };
 return {db,reset,snapshot,call,close:()=>db.close()};
}
