import {Database} from 'bun:sqlite';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {invokeSync} from '../host/driver.ts';
import {createStorage} from '../storage/adapter.ts';
import {bunSqlite} from '../storage/bun-sqlite.ts';
const root=resolve(import.meta.dir,'../../..');
const spec=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/acceptance.json','utf8'));
const program=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const module=new WebAssembly.Module(readFileSync(root+'/experiments/wasm-exp1/rust-full/build/probe.wasm'));
const entry=program.declarations.find((d:any)=>d.name==='probe');
const db=new Database(root+'/build/wasm-exp1/optimization/attribute.sqlite');
const storage=createStorage(program,bunSqlite(db));storage.setup();storage.resetFixture({Item:spec.seeds.Item});
const row=spec.seeds.Item[0],principal={entity:'User',values:{id:row.owner_id}};
const input={id:row.id,title:'fallback',note:''};input.note='x'.repeat(256-JSON.stringify(input).length);
let args:any;invokeSync(module,entry.semanticId,input,(cap,a)=>{args=structuredClone(a);return {kind:'success',value:row};});
const sql='SELECT "id", "owner_id", "title", "note" FROM "entity_Item" WHERE "id" = ? AND ("owner_id" = ?) LIMIT 2';
const statement=db.prepare(sql);const enc=new TextEncoder(),dec=new TextDecoder();
const methods:any={
 sqlite_prepared:()=>statement.all(row.id,row.owner_id)[0]?.title==='alpha',
 sqlite_prepare_each:()=>db.prepare(sql).all(row.id,row.owner_id)[0]?.title==='alpha',
 host_adapter:()=>storage.read(args,principal).value.title==='alpha',
 instantiate_only:()=>new WebAssembly.Instance(module).exports.memory instanceof WebAssembly.Memory,
 host_json_roundtrip:()=>JSON.parse(dec.decode(enc.encode(JSON.stringify({kind:'success',value:row})))).value.title==='alpha',
 wasm_fresh_mock:()=>{const r=invokeSync(module,entry.semanticId,input,()=>({kind:'success',value:row}));return r.kind==='success'&&r.value==='alpha';},
 wasm_fresh_sql:()=>{const r=invokeSync(module,entry.semanticId,input,(_c,a)=>storage.read(a,principal));return r.kind==='success'&&r.value==='alpha';},
};
const results:any[]=[];const names=Object.keys(methods);
for(let repeat=0;repeat<3;repeat++)for(const name of [...names.slice(repeat),...names.slice(0,repeat)]){
 const call=methods[name];let count=0;const samples:number[]=[];
 for(const record of [false,true]){const start=performance.now(),end=start+(record?1000:200);let batch=0;
 while(performance.now()<end){const before=performance.now();if(!call())throw Error(name+' semantic mismatch');if(record){samples.push(performance.now()-before);count++;}if(++batch%256===0)await Bun.sleep(0);}
 if(record){const seconds=(performance.now()-start)/1000;samples.sort((a,b)=>a-b);const q=(n:number)=>samples[Math.floor((samples.length-1)*n)];const result={run:repeat+1,name,requests:count,seconds,throughput:count/seconds,p50Ms:q(.5),p95Ms:q(.95),p99Ms:q(.99),errors:0};results.push(result);console.log(JSON.stringify(result));}
 }
}
writeFileSync(root+'/build/wasm-exp1/optimization/attribution.json',JSON.stringify({scope:'Stage microbenchmarks; stages not additive due allocation/GC/scheduling differences',results},null,2)+'\n');db.close();
