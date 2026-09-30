import {Miniflare,convertV4MiniflareOptions,Log,LogLevel} from '../tooling/node_modules/miniflare/dist/src/index.js';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {randomUUID} from 'node:crypto';
import {Agent,request} from 'node:http';
import assert from 'node:assert/strict';
const root=resolve(import.meta.dirname,'../../..'),out=root+'/build/wasm-exp1/workerd-native-values';mkdirSync(out,{recursive:true});
const bundle=out+'/bundle',token=randomUUID(),manifest=JSON.parse(readFileSync(bundle+'/manifest.json'));
const spec=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/acceptance.json'));
const agent=new Agent({keepAlive:true,maxSockets:16});
const mf=new Miniflare(convertV4MiniflareOptions({log:new Log(LogLevel.WARN),handleUncaughtError:error=>console.error(error.stack),name:'local-read',host:'127.0.0.1',port:0,cf:false,compatibilityDate:'2026-09-30',logRequests:false,telemetry:{enabled:false},resourcePersistencePath:out+'/state-'+randomUUID(),modulesRoot:bundle,modules:[{type:'ESModule',path:bundle+'/worker.mjs',contents:readFileSync(bundle+'/worker.mjs','utf8')},...['candidate','typed','previous','views','fused','adaptive','bounded','direct','hybrid','scan'].map(n=>({type:'CompiledWasm',path:bundle+'/'+n+'.wasm',contents:readFileSync(bundle+'/'+n+'.wasm')}))],durableObjects:{READ_AUTHORITY:{className:'ReadAuthority',useSQLite:true}},bindings:{EXPERIMENT_TOKEN:token}}));
const profiling=process.argv.includes('--profile'),micro=process.argv.includes('--micro');
const smoke=process.argv.includes('--smoke'),results=[],preflight=[];
const protocol={repetitions:smoke?1:5,concurrency:smoke?[1]:[1,16],warmupSeconds:smoke?.05:.5,measureSeconds:smoke?.1:2,targets:micro?['js','views','hybrid','noop']:['js','views','hybrid','noop'],workloads:['small','large','escaped','unicode','late'],scope:'Local workerd HTTP qualification of selected sampled typed host driver versus frozen views driver and generated JS, with the identical WASM module and authority adapter. 0.5s warmup/2s measurement; descriptive pairs, not production capacity. Escaped fixture: 512 repeats, same bytes as isolated selection.'};
const save=()=>writeFileSync(out+(profiling?'/profile.json':micro?'/micro.json':smoke?'/smoke.json':'/results.json'),JSON.stringify({protocol,manifest,preflight,results},null,2)+'\n');
try{
 const url=await mf.ready;
 const call=(data)=>new Promise((resolve,reject)=>{const body=JSON.stringify(data),r=request(url,{method:'POST',agent,headers:{'content-type':'application/json','content-length':Buffer.byteLength(body),'x-experiment-token':token}},res=>{const chunks=[];res.on('data',b=>chunks.push(b));res.on('error',reject);res.on('end',()=>{try{assert.equal(res.statusCode,200,Buffer.concat(chunks).toString().slice(0,2000));resolve(JSON.parse(Buffer.concat(chunks).toString()));}catch(e){reject(e);}});});r.on('error',reject);r.setTimeout(10000,()=>r.destroy(Error('HTTP timeout')));r.end(body);});
 if(profiling){
  for(const size of [256,16384,49152])for(const kind of ['ascii','escaped'])for(const operation of ['Item.read','Item.read_title'])for(const flags of [14,15]){
   const options={size,kind,operation,flags},start=performance.now();
   const report=await call({action:'profile',key:'run-profile',options});
   results.push({options,externalMs:performance.now()-start,...report});save();console.log(JSON.stringify(results.at(-1)));
  }
 } else {
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
 if(micro){
  protocol.workloads=['small','large','escaped','large48','unicode','late'];
  protocol.scope='Isolated in-Object selection, external elapsed time per verified call. 200 warmup and 2000 measured calls per cell; not HTTP capacity.';
  for(let run=0;run<5;run++)for(const workload of (run%2?['late','unicode','large48','escaped','large','small']:['small','large','escaped','large48','unicode','late'])){
   const targets=[...protocol.targets.slice(run%protocol.targets.length),...protocol.targets.slice(0,run%protocol.targets.length)];
   for(const target of targets){
    const key=`run-micro-${run}-${target}`,note=workload==='small'?a.note:workload==='escaped'?'é中😀"\\\n'.repeat(512):workload==='late'?'x'.repeat(8192)+'é中😀"\\\n'.repeat(512):workload==='unicode'?'é中😀'.repeat(2048):'x'.repeat(workload==='large48'?49152:16384);
    await call({action:'reset',key,note});
    const operation=workload==='small'?'probe':'Item.read',input=workload==='small'?{id:a.id,title:'fallback'}:a.id;
    const data={target,operation,input};
    const expected={kind:'success',value:workload==='small'?a.title:{...a,note}};
    const warm=await call({action:'batch',key,call:data,iterations:200});assert.deepEqual(warm.last,expected);
    await call({action:'begin',key});const start=performance.now();
    const result=await call({action:'batch',key,call:data,iterations:2000});const externalMs=performance.now()-start;
    assert.deepEqual(result.last,expected);assert.equal(result.iterations,2000);
    const stats=await call({action:'stats',key});assert.equal(stats.count,2000);assert.deepEqual(stats.snapshot,{Item:[{...a,note},b,c]});
    results.push({target,run:run+1,workload,iterations:2000,frameBytes:Buffer.byteLength(JSON.stringify(expected)),externalUs:externalMs/2,localUs:result.elapsedMs/2,snapshotVerified:true,poolStats:stats.poolStats,newPoolStats:stats.newPoolStats,nextPoolStats:stats.nextPoolStats,hybridPoolStats:stats.hybridPoolStats});save();console.log(JSON.stringify(results.at(-1)));
   }
  }
 } else {
 for(const concurrency of protocol.concurrency)for(let run=0;run<protocol.repetitions;run++){
  const ts=protocol.targets,targets=[...ts.slice(run%ts.length),...ts.slice(0,run%ts.length)];
  for(const target of targets)for(const workload of run%2?[...protocol.workloads].reverse():protocol.workloads){
   const key=`run-${concurrency}-${run}-${target}`,large=workload!=='small',note=workload==='escaped'?'é中😀"\\\n'.repeat(512):workload==='late'?'x'.repeat(8192)+'é中😀"\\\n'.repeat(512):workload==='unicode'?'é中😀'.repeat(2048):large?'x'.repeat(16384):a.note;
   await call({action:'reset',key,note});
   const input={id:a.id,title:'fallback',note:''};input.note='x'.repeat(256-JSON.stringify(input).length);
   const wire={action:'call',key,call:{target,operation:large?'Item.read':'probe',input:large?a.id:input}};
   const expected={kind:'success',value:large?{...a,note}:a.title};let count=0,errors=0;const latency=[];
   const phase=async(seconds,record)=>{const end=performance.now()+seconds*1000;await Promise.all(Array.from({length:concurrency},async()=>{while(performance.now()<end){const t=performance.now();const actual=await call(wire);assert.deepEqual(actual,expected);if(record){count++;latency.push(performance.now()-t);}}}));};
   await phase(protocol.warmupSeconds,false);await call({action:'begin',key});const start=performance.now();await phase(protocol.measureSeconds,true);const elapsed=performance.now()-start,stats=await call({action:'stats',key});
   assert.equal(stats.count,count);assert.deepEqual(stats.snapshot,{Item:[{...a,note},b,c]});latency.sort((a,b)=>a-b);
   const result={target,concurrency,run:run+1,workload,count,errors,throughput:count*1000/elapsed,p95Ms:latency[Math.floor(latency.length*.95)],p99Ms:latency[Math.floor(latency.length*.99)],maxMs:latency.at(-1),snapshotVerified:true,poolStats:stats.poolStats,newPoolStats:stats.newPoolStats,nextPoolStats:stats.nextPoolStats,hybridPoolStats:stats.hybridPoolStats};results.push(result);save();console.log(JSON.stringify(result));
  }
 }
}}}catch(error){save();writeFileSync(out+'/failure.json',JSON.stringify({error:String(error),stack:error.stack,completedCells:results.length},null,2)+'\n');throw error;}
finally{agent.destroy();await mf.dispose();}
