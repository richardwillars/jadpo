import {Database} from 'bun:sqlite';
import {resolve} from 'node:path';
import {createSliceHost} from './cached-host.ts';
import {bunSqlite} from '../optimization/cached-sqlite.ts';
const root=resolve(import.meta.dir,'../../..')+'/';
const module=await WebAssembly.compile(await Bun.file(root+'experiments/wasm-exp1/boundary-http/compiler/build/application.wasm').arrayBuffer());
const p=await Bun.file(root+'experiments/wasm-exp1/compiler/build/projected/program.json').json();
const a=await Bun.file(root+'experiments/wasm-exp1/acceptance.json').json();
const rows=a.seeds.Item,results=[];
const canonical=(x:any):string=>JSON.stringify(x,(_k,v)=>v&&typeof v==='object'&&!Array.isArray(v)?Object.fromEntries(Object.entries(v).sort(([a],[b])=>a.localeCompare(b))):v);
for(const mode of ['throw-second','malformed-second-row','oversized-second-row','wrong-principal-entity']) {
 const db=new Database(':memory:');const adapter=bunSqlite(db);let updates=0;
 const traced={...adapter,rows:(sql:string,params:any[])=>{
  if(sql.startsWith('UPDATE ')) {
   updates++;
   if(updates===2&&mode==='throw-second')throw new Error('REVIEW_SECRET_SENTINEL');
   const value=adapter.rows(sql,params);
   if(updates===2&&mode==='malformed-second-row')return value.map(r=>({...r,title:1}));
   if(updates===2&&mode==='oversized-second-row')return value.map(r=>({...r,note:'x'.repeat(65536)}));
   return value;
  }
  return adapter.rows(sql,params);
 }};
 const spec=structuredClone(a);spec.seeds.principals.impostor={entity:'Item',id:a.seeds.principals.owner.id};
 const host=createSliceHost(module,p,spec,traced as any,{reopen:async()=>null});
 await host.reset();const before=await host.snapshot();
 const actual=await host.invoke('update_pair',[rows[0].id,'first-new',rows[1].id,'second-new'],mode==='wrong-principal-entity'?'impostor':'owner');
 const after=await host.snapshot();
 const expected=mode==='wrong-principal-entity'?'domain':'internal';
 results.push({mode,pass:actual.output.kind===expected&&canonical(before)===canonical(after),actual,before,after,updates});db.close();
}
await Bun.write(new URL('../build/review-atomic-results.json',import.meta.url),JSON.stringify(results,null,2));
console.log(JSON.stringify(results.map(x=>({mode:x.mode,pass:x.pass,output:x.actual.output,events:x.actual.events,updates:x.updates}))));
if(results.some(x=>!x.pass))process.exitCode=1;
