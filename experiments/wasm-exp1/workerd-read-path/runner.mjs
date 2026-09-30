import {Miniflare,convertV4MiniflareOptions,Log,LogLevel} from '../tooling/node_modules/miniflare/dist/src/index.js';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {randomUUID} from 'node:crypto';
import {Agent,request} from 'node:http';
import assert from 'node:assert/strict';
const root=resolve(import.meta.dirname,'../../..'),out=root+'/build/wasm-exp1/workerd-read-path';mkdirSync(out,{recursive:true});
const bundle=out+'/bundle',token=randomUUID(),manifest=JSON.parse(readFileSync(bundle+'/manifest.json'));
const spec=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/acceptance.json'));
const agent=new Agent({keepAlive:true,maxSockets:16});
const mf=new Miniflare(convertV4MiniflareOptions({log:new Log(LogLevel.WARN),handleUncaughtError:error=>console.error(error.stack),name:'local-read',host:'127.0.0.1',port:0,cf:false,compatibilityDate:'2026-09-30',logRequests:false,telemetry:{enabled:false},resourcePersistencePath:out+'/state-'+randomUUID(),modulesRoot:bundle,modules:[{type:'ESModule',path:bundle+'/worker.mjs',contents:readFileSync(bundle+'/worker.mjs','utf8')},...['candidate','typed','previous'].map(n=>({type:'CompiledWasm',path:bundle+'/'+n+'.wasm',contents:readFileSync(bundle+'/'+n+'.wasm')}))],durableObjects:{READ_AUTHORITY:{className:'ReadAuthority',useSQLite:true}},bindings:{EXPERIMENT_TOKEN:token}}));
const smoke=process.argv.includes('--smoke'),results=[],preflight=[];
const protocol={repetitions:smoke?1:5,concurrency:smoke?[1]:[1,16],warmupSeconds:smoke?.05:.5,measureSeconds:smoke?.1:2,targets:['js','previous','candidate','typed','noop'],workloads:['small','large'],scope:'Local workerd/V8 + SQLite Durable Object, external Node HTTP generator. No Bun runtime. Same generated JS read functions with a shared authority adapter. Short exploratory timing, not the Bun parity qualification or production capacity.'};
const save=()=>writeFileSync(out+(smoke?'/smoke.json':'/results.json'),JSON.stringify({protocol,manifest,preflight,results},null,2)+'\n');
try{
 const url=await mf.ready;
 const call=(data)=>new Promise((resolve,reject)=>{const body=JSON.stringify(data),r=request(url,{method:'POST',agent,headers:{'content-type':'application/json','content-length':Buffer.byteLength(body),'x-experiment-token':token}},res=>{const chunks=[];res.on('data',b=>chunks.push(b));res.on('error',reject);res.on('end',()=>{try{assert.equal(res.statusCode,200,Buffer.concat(chunks).toString().slice(0,2000));resolve(JSON.parse(Buffer.concat(chunks).toString()));}catch(e){reject(e);}});});r.on('error',reject);r.setTimeout(10000,()=>r.destroy(Error('HTTP timeout')));r.end(body);});
 const [a,b,c]=spec.seeds.Item;
 for(const target of protocol.targets.filter(t=>t!=='noop')){
  const key='run-preflight-'+target,reset=await call({action:'reset',key,note:'x'.repeat(16384)});
  assert.equal(reset.runtime.hasBun,false); // A Cloudflare process compatibility global is not a Node host.
  assert.equal(manifest.noBunOrNodeRuntimeDependency,true);
  const invoke=(operation,input,principal='owner')=>call({action:'call',key,call:{target,operation,input,principal}});
  assert.deepEqual(await invoke('Item.read',a.id),{kind:'success',value:{...a,note:'x'.repeat(16384)}});
  assert.deepEqual(await invoke('Item.read',a.id,'other'),{kind:'domain',failure:'ItemMissing'});
  assert.deepEqual(await invoke('Item.read',spec.seeds.missing_item_id),{kind:'domain',failure:'ItemMissing'});
  assert.deepEqual(await invoke('Item.read','bad'),{kind:'invalid'});
  assert.deepEqual(await invoke('probe',{id:spec.seeds.missing_item_id,title:'fallback'}),{kind:'success',value:'fallback'});
  await call({action:'changeOwner',key});
  assert.deepEqual(await invoke('Item.read',a.id),{kind:'domain',failure:'ItemMissing'});
  assert.deepEqual(await invoke('Item.read',a.id,'other'),{kind:'success',value:{...a,note:'x'.repeat(16384),owner_id:spec.seeds.principals.other.id}});
  preflight.push({target,passed:7,...reset});save();
 }
 for(const concurrency of protocol.concurrency)for(let run=0;run<protocol.repetitions;run++){
  const ts=protocol.targets,targets=[...ts.slice(run%ts.length),...ts.slice(0,run%ts.length)];
  for(const target of targets)for(const workload of run%2?['large','small']:['small','large']){
   const key=`run-${concurrency}-${run}-${target}`,large=workload==='large',note=large?'x'.repeat(16384):a.note;
   await call({action:'reset',key,note});
   const input={id:a.id,title:'fallback',note:''};input.note='x'.repeat(256-JSON.stringify(input).length);
   const wire={action:'call',key,call:{target,operation:large?'Item.read':'probe',input:large?a.id:input}};
   const expected={kind:'success',value:large?{...a,note}:a.title};let count=0,errors=0;const latency=[];
   const phase=async(seconds,record)=>{const end=performance.now()+seconds*1000;await Promise.all(Array.from({length:concurrency},async()=>{while(performance.now()<end){const t=performance.now();const actual=await call(wire);assert.deepEqual(actual,expected);if(record){count++;latency.push(performance.now()-t);}}}));};
   await phase(protocol.warmupSeconds,false);await call({action:'begin',key});const start=performance.now();await phase(protocol.measureSeconds,true);const elapsed=performance.now()-start,stats=await call({action:'stats',key});
   assert.equal(stats.count,count);assert.deepEqual(stats.snapshot,{Item:[{...a,note},b,c]});latency.sort((a,b)=>a-b);
   const result={target,concurrency,run:run+1,workload,count,errors,throughput:count*1000/elapsed,p95Ms:latency[Math.floor(latency.length*.95)],p99Ms:latency[Math.floor(latency.length*.99)],maxMs:latency.at(-1),snapshotVerified:true,poolStats:stats.poolStats};results.push(result);save();console.log(JSON.stringify(result));
  }
 }
}catch(error){save();writeFileSync(out+'/failure.json',JSON.stringify({error:String(error),stack:error.stack,completedCells:results.length},null,2)+'\n');throw error;}
finally{agent.destroy();await mf.dispose();}
