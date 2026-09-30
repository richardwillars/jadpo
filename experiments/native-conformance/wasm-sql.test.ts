import {test,expect} from 'bun:test';
import {Database} from 'bun:sqlite';
import {readFileSync} from 'node:fs';
import {createStorage} from '../wasm-exp1/boundary-http/adapter.ts';
import {bunSqlite} from '../wasm-exp1/optimization/cached-sqlite.ts';
import {loadGuest} from './wasm-driver.ts';
const program=JSON.parse(readFileSync(import.meta.dir+'/build/program.json','utf8'));
const manifest=JSON.parse(readFileSync(import.meta.dir+'/build/manifest.json','utf8'));
const config=JSON.parse(readFileSync(import.meta.dir+'/build/storage.json','utf8'));
test('cancelled WASM second effect rolls back the already-executed first SQLite write',()=>{
 const db=new Database(':memory:');const storage=createStorage(program,bunSqlite(db));storage.setup();
 const read=config.plans.find((p:any)=>p.effect==='read'),owner='11111111-1111-4111-8111-111111111111';
 const a='aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',b='bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
 const rows=[{id:a,[read.scope]:owner,title:'alpha',note:null},{id:b,[read.scope]:owner,title:'bravo',note:null}];storage.resetFixture({[config.entity]:rows});
 const g=loadGuest(),pair=manifest.entries.find((e:any)=>e.name==='update_pair');
 const result=storage.runAtomic(()=>{
  const first=g.start(pair.semanticId,[a,'first-new',b,'second-new']);expect(first.kind).toBe('pending');
  const reply=storage.update(first.args,{entity:read.principal,values:{id:owner}});
  expect(storage.snapshot()[config.entity][0].title).toBe('first-new');
  const second=g.resume(first,reply);expect(second.operationId).toBe(2);return g.cancel();
 });
 expect(result).toEqual({kind:'internal'});expect(storage.snapshot()[config.entity]).toEqual(rows);db.close();
});
