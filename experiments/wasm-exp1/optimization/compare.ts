// Repeated full-boundary workload; execute targets sequentially, never together.
import {Database} from 'bun:sqlite';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {createStorage} from '../storage/adapter.ts';
import {bunSqlite} from '../storage/bun-sqlite.ts';
import {invoke,invokeSync} from '../host/driver.ts';
import {invokeSync as pooled,poolStats} from './driver.ts';
import {createStorage as createCachedStorage} from './cached-adapter.ts';
import {bunSqlite as cachedSqlite} from './cached-sqlite.ts';
const root=resolve(import.meta.dir,'../../..');
const out=join(root,'build/wasm-exp1/optimization/comparison');await mkdir(out,{recursive:true});
const spec=JSON.parse(await readFile(join(root,'experiments/wasm-exp1/acceptance.json'),'utf8'));
const program=JSON.parse(await readFile(join(root,'experiments/wasm-exp1/compiler/build/projected/program.json'),'utf8'));
const workload={concurrency:[1,8],runsPerWorkloadPerConcurrency:5,payloadBytes:256,warmupSeconds:1,measureSeconds:3};
const wasmPath=resolve(Bun.argv[2]??'experiments/wasm-exp1/rust-full/build/full.wasm');
const baselinePath=join(root,'build/wasm-exp1/baseline/generated/bun/target');
const bytes=await readFile(wasmPath),module=await WebAssembly.compile(bytes);
const originalBytes=await readFile(join(root,'experiments/wasm-exp1/rust-full/build/probe.wasm')),originalModule=await WebAssembly.compile(originalBytes);
const hash=(x:Uint8Array|string)=>createHash('sha256').update(x).digest('hex');
const entry=program.declarations.find((d:any)=>d.name===spec.scope.probe_entrypoint);
const query=program.declarations.find((d:any)=>d.kind==='callable'&&d.body[0]?.value?.value?.op==='query');
if(!entry||!query)throw new Error('Checked probe/query missing');
const q=query.body[0].value.value;
const entity=q.entity;
const binding=program.policy.bindings.find((b:any)=>b.entity===entity);
const principal=spec.seeds.principals.owner;
const row=spec.seeds[entity][0];
const db=new Database(join(out,'wasm.sqlite'),{strict:true});
const storage=createStorage(program,bunSqlite(db));storage.setup();storage.resetFixture({[entity]:spec.seeds[entity]});
const preparedStorage=createStorage(program,cachedSqlite(db));
const cachedStorage=createCachedStorage(program,cachedSqlite(db));
delete Bun.env.DATABASE_URL;Bun.env.SQLITE_PATH=join(out,'bun.sqlite');
const app=await import(join(baselinePath,'app.ts'));
const {persistence}=await import(join(baselinePath,'persistence.ts'));
const baselineDb=new Database(Bun.env.SQLITE_PATH,{strict:true});
// Generated SQLite schema uses the compiler's physical table identifier. Obtain
// it from its own sqlite_schema, requiring a unique exact field-shape match.
const names=baselineDb.query("SELECT name FROM sqlite_schema WHERE type='table'").all() as {name:string}[];
const fields=Object.keys(row);
const matchingTables=names.filter(({name})=>{
 if(!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name))return false;
 const columns=baselineDb.query(`PRAGMA table_info("${name}")`).all() as {name:string}[];
 return columns.length===fields.length&&columns.every(c=>fields.includes(c.name));
});
if(matchingTables.length!==1)throw new Error('No unique baseline fixture table');
const table=matchingTables[0].name;
baselineDb.exec(`DELETE FROM "${table}"`);
for(const value of spec.seeds[entity])baselineDb.prepare(`INSERT INTO "${table}" (${fields.map(f=>`"${f}"`).join(',')}) VALUES (${fields.map(()=>'?').join(',')})`).run(...fields.map(f=>value[f]));
const encoder=new TextEncoder(),decoder=new TextDecoder();
const wire=(value:any)=>JSON.parse(decoder.decode(encoder.encode(JSON.stringify(value))));
const trusted={entity:binding.principalEntity,values:{[program.entities.find((e:any)=>e.name===binding.principalEntity).identity]:principal.id}};
// This baseline-only mapping is the existing generated target's trusted fixture
// convention; it does not claim an authentication provider.
const bunPrincipal={kind:'user',subject:principal.id,values:{user_id:principal.id}};
const method=`query_required_${entity}_by_${q.predicate.field}`;
const mock:any={withPolicy(){return this},[method]:async()=>structuredClone(row)};
const sample=(size:number)=>{
 const input:any={id:row.id,title:'fallback',note:''};
 const overhead=encoder.encode(JSON.stringify(input)).length;
 input.note='x'.repeat(size-overhead);
 if(encoder.encode(JSON.stringify(input)).length!==size)throw new Error('Payload size');
 return input;
};
async function call(target:string,_mode:string,input:any){
 if(target==='bun'){
  const parsed=app.experimentValidators[entry.parameters[0].type.name](wire(input),'input');
  const value=await app.experimentCallables[entry.name](parsed,app.captureOperation(null,bunPrincipal),persistence);
  return wire({kind:'success',value});
 }
 const selected=target==='fresh_original'?originalModule:module;
 const driver=target.startsWith('pooled')?pooled:invokeSync;
 const selectedStorage=target==='pooled_prepared'?preparedStorage:target==='pooled_cached'?cachedStorage:storage;
 return driver(selected,entry.semanticId,input,(capability,args)=>{
  if(capability!=='storage.read')throw new Error('Unexpected benchmark capability');
  return target.endsWith('_mock')?{kind:'success',value:structuredClone(row)}:selectedStorage.read(args,trusted);
 });
}
const quantile=(xs:number[],p:number)=>xs[Math.min(xs.length-1,Math.floor((xs.length-1)*p))]??null;
async function run(target:string,mode:string,concurrency:number,size:number,warmupSeconds:number,measureSeconds:number){
 const input=sample(size);let errors=0,count=0;const latencies:number[]=[];
 async function phase(seconds:number,record:boolean){
  const end=performance.now()+seconds*1000;
  await Promise.all(Array.from({length:concurrency},async()=>{
   let batch=0;
   while(performance.now()<end){
    const started=performance.now();let ok=false;
    try{const result=await call(target,mode,input);ok=result.kind==='success'&&result.value==='alpha';}catch{}
    if(!ok&&!record)throw new Error('Warmup semantic failure');
    if(record){count++;if(!ok)errors++;latencies.push(performance.now()-started);}
    // Let timers and engine housekeeping progress on both targets equally.
    if(++batch%256===0)await Bun.sleep(0);
   }
  }));
 }
 await phase(warmupSeconds,false);
 const started=performance.now();await phase(measureSeconds,true);const seconds=(performance.now()-started)/1000;
 latencies.sort((a,b)=>a-b);
 return {target,mode,concurrency,payloadBytes:size,seconds,requests:count,errors,successfulThroughput:(count-errors)/seconds,p50Ms:quantile(latencies,.5),p95Ms:quantile(latencies,.95),p99Ms:quantile(latencies,.99),rssBytes:process.memoryUsage().rss,latenciesScope:'All requests retained only as in-memory samples for quantiles; run aggregates persisted'};
}
const targets=['bun','fresh_original','fresh_reset','pooled','pooled_prepared','pooled_cached','fresh_mock','pooled_mock'];
const results:any[]=[];
for(const concurrency of workload.concurrency)for(let repetition=0;repetition<workload.runsPerWorkloadPerConcurrency;repetition++){
 const order=[...targets.slice(repetition),...targets.slice(0,repetition)];
 for(const target of order){
  const result={run:repetition+1,...await run(target,target.endsWith('_mock')?'mock':'io',concurrency,256,workload.warmupSeconds,workload.measureSeconds)};
  results.push(result);console.log(JSON.stringify(result));
  await writeFile(join(out,'results.json'),JSON.stringify({originalWasmHash:hash(originalBytes),wasmHash:hash(bytes),baselineHash:hash(await readFile(join(baselinePath,'app.ts'))),runnerHash:hash(await readFile(import.meta.path)),workload,order:'Rotate start target by repetition; targets run sequentially',poolStats,results},null,2)+'\n');
  if(result.errors)throw Error('Semantic workload failure');
 }
}
baselineDb.close();db.close();
