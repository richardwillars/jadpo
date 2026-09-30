import {launch,a,b,c,out,q,base} from './harness.mjs';
import assert from 'node:assert/strict';
import {writeFileSync,readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
const smoke=process.argv.includes('--smoke'),startup=process.argv.includes('--startup'),micro=process.argv.includes('--micro'),deletes=process.argv.includes('--delete');
const mode=startup?'startup':micro?'micro':deletes?'delete':smoke?'smoke':'http';
const evidence={protocol:{mode,bunPeakRssDivisor:1,repetitions:startup?20:smoke?1:5,warmupSeconds:smoke?.05:.5,measureSeconds:smoke?.1:2,concurrency:[1,16],journal:deletes?'DELETE':'WAL',scope:'local same-machine closed-loop Node client; full verification; no retries'},hashes:Object.fromEntries(['src/main.rs','src/shared.rs','src/storage.rs','bun-server.ts','measure.mjs','build/generated.rs'].map(p=>[p,createHash('sha256').update(readFileSync(base+'/'+p)).digest('hex')]).concat([['executable',createHash('sha256').update(readFileSync(out+'/target/release/jadpo-native-exp1')).digest('hex')]])),servers:[],results:[]};
const save=()=>writeFileSync(out+'/'+mode+'.json',JSON.stringify(evidence,null,2)+'\n');
const noteFor=work=>work.includes('unicode')?'é😀'.repeat(3000):work.includes('large')?'x'.repeat(16384):work.includes('48k')?'x'.repeat(49152):work.includes('escaped')?'x'.repeat(8192)+'\n"\\'.repeat(500):undefined;
let active;
try {
for(let rep=0;rep<evidence.protocol.repetitions;rep++)for(const concurrency of startup||micro?[1]:evidence.protocol.concurrency)for(const target of rep%2?['native','bun']:['bun','native']){
 const s=active=await launch(target,evidence.protocol.journal);evidence.servers.push({run:rep+1,target,concurrency,...s.info});
 for(const [k,v] of Object.entries({journal_mode:evidence.protocol.journal.toLowerCase(),synchronous:2,foreign_keys:1,fullfsync:0,wal_autocheckpoint:1000,busy_timeout:0}))assert.equal(s.info.pragmas[k],v,k);
 if(startup){const t=performance.now();assert.deepEqual((await s.send('call',{operation:'Item.read',input:a.id})).body,{kind:'success',value:a});const firstHttpMs=performance.now()-t,processToFirstResponseMs=performance.now()-s.started;const warm=[];for(let i=0;i<20;i++){const t=performance.now();assert.deepEqual((await s.send('call',{operation:'Item.read',input:a.id})).body,{kind:'success',value:a});warm.push(performance.now()-t);}evidence.results.push({target,run:rep+1,readyMs:s.readyMs,firstHttpMs,processToFirstResponseMs,warmP50Ms:q(warm.sort((a,b)=>a-b),.5),rssBytes:s.rss()});save();await s.stop();active=null;continue;}
 const workloads=deletes?['write_single','write_pair']:micro?['read_small','read_large','read_unicode','read_48k','read_escaped','title_large']:['read_small','read_large','read_unicode','write_single','write_pair','noop_small','noop_large','noop_unicode'];if(rep%2)workloads.reverse();
 for(const workload of workloads){
  const note=noteFor(workload);await s.control('reset',note===undefined?{}:{note});
  const small=workload.includes('small'),pair=workload==='write_pair',single=workload==='write_single',noop=workload.startsWith('noop');
  const operation=small?'probe':single?'Item.change':pair?'update_pair':workload==='title_large'?'Item.read_title':'Item.read';
  const inputFor=(lane,sequence)=>small?{id:a.id,title:'fallback',note:'x'.repeat(169)}:single?[a.id,{note:`${lane}-${sequence}`+'x'.repeat(240)}]:pair?[a.id,`one${lane}-${sequence%2}`,b.id,`two${lane}-${sequence%2}`]:a.id;
  const expectedFor=input=>({kind:'success',value:small||workload==='title_large'?'alpha':single?{...a,note:input[1].note}:pair?{...b,title:input[3]}:{...a,...(note===undefined?{}:{note})}});
  if(micro){const input=inputFor(0,0),body={operation,input,expected:expectedFor(input)};await s.control('micro',{...body,count:200});const result=await s.control('micro',{...body,count:2000});assert.equal(result.count,2000);evidence.results.push({target,run:rep+1,workload,...result});save();continue;}
  let count=0,errors=0,firstError=null,latencies=[],markers=new Set();
  const phase=async(seconds,record)=>{const end=performance.now()+seconds*1000;await Promise.all(Array.from({length:concurrency},async(_,lane)=>{let sequence=0;while(performance.now()<end){const input=inputFor(lane,sequence++),start=performance.now();let ok=false;try{const result=await s.send(noop?'noop':'call',{operation,input});assert.equal(result.status,200);assert.deepEqual(result.body,expectedFor(input));ok=true;if(record&&single)markers.add(input[1].note);}catch(e){firstError??=String(e);}if(record){count++;errors+=ok?0:1;latencies.push(performance.now()-start);}else if(!ok)throw Error(firstError);}}));};
  await phase(evidence.protocol.warmupSeconds,false);await s.control('begin');const rssStart=s.rss(),cpu=process.cpuUsage(),start=performance.now();await phase(evidence.protocol.measureSeconds,true);const elapsedMs=performance.now()-start,clientCpu=process.cpuUsage(cpu),stats=await s.control('stats');assert.equal(stats.count,count);
  if(single){assert(markers.has(stats.snapshot[0].note));assert.deepEqual({...stats.snapshot[0],note:a.note},a);assert.deepEqual(stats.snapshot.slice(1),[b,c]);}
  else if(pair){assert(/^one\d+-[01]$/.test(stats.snapshot[0].title));assert.equal(stats.snapshot[1].title,'two'+stats.snapshot[0].title.slice(3));assert.deepEqual({...stats.snapshot[0],title:a.title},a);assert.deepEqual({...stats.snapshot[1],title:b.title},b);assert.deepEqual(stats.snapshot[2],c);}
  else assert.deepEqual(stats.snapshot,[{...a,...(note===undefined?{}:{note})},b,c]);
  latencies.sort((a,b)=>a-b);const result={target,run:rep+1,concurrency,workload,count,errors,firstError,elapsedMs,throughput:count*1000/elapsedMs,p50Ms:q(latencies,.5),p95Ms:q(latencies,.95),p99Ms:q(latencies,.99),maxMs:latencies.at(-1),serverCpuUs:stats.cpu.user+stats.cpu.system,cpuUsPerRequest:(stats.cpu.user+stats.cpu.system)/count,clientCpu,rssStartBytes:rssStart,rssEndBytes:s.rss(),peakRssBytes:stats.maxRssBytes,snapshotVerified:true};evidence.results.push(result);save();console.log(JSON.stringify(result));assert.equal(errors,0);
 }
 await s.stop();active=null;
}
}catch(e){evidence.failure={message:String(e),stderr:active?.stderr()};save();throw e;}finally{if(active)await active.stop();}
save();console.log(JSON.stringify({mode,cells:evidence.results.length,count:evidence.results.reduce((n,x)=>n+(x.count??1),0)}));
