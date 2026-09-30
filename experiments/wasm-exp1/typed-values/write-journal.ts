import {Database} from 'bun:sqlite';
import {environment,originalRows} from './environment.ts';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const out='build/wasm-exp1/typed-values/write-journal';mkdirSync(out,{recursive:true});
const results:any[]=[];const [a,b]=originalRows;
const targets=['previous','candidate'];
const q=(xs:number[],p:number)=>xs.slice().sort((a,b)=>a-b)[Math.floor((xs.length-1)*p)];
for(let rep=0;rep<5;rep++)for(const mode of (rep%2?['wal','delete']:['delete','wal']))for(const target of (rep%2?[...targets].reverse():targets)){
 const env=await environment(target,`${out}/${target}-${mode}-${rep}.sqlite`,true);
 let finalSnapshot:any;
 try{
  env.db.exec(mode==='wal'?'PRAGMA journal_mode = WAL':'PRAGMA journal_mode = DELETE');
  const journal=env.db.query('PRAGMA journal_mode').get() as any;
  assert.equal(journal.journal_mode,mode);
  assert.equal((env.db.query('PRAGMA synchronous').get() as any).synchronous,2);
  const sqliteVersion=env.db.query('SELECT sqlite_version() AS version').get();
  const fullfsync=env.db.query('PRAGMA fullfsync').get();
  env.reset();const before=env.snapshot();
  assert.deepEqual(await env.call('update_pair',[a.id,'first',b.id,'charlie']),{kind:'domain',failure:'ItemConflict'});
  assert.deepEqual(env.snapshot(),before);

  for(const workload of (rep%2?['pair','single']:['single','pair'])){
   env.reset();let count=0;const latency:number[]=[];
   for(const record of [false,true]){
    env.timing!.reset();const start=performance.now(),end=start+(record?1500:200);
    while(performance.now()<end){
     const n=count%2,title1='one'+n,title2='two'+n,note='value'+n+'x'.repeat(250);
     const t=performance.now();
     const result=workload==='single'?await env.call('Item.change',[a.id,{note}]):await env.call('update_pair',[a.id,title1,b.id,title2]);
     if(record)latency.push(performance.now()-t);
     assert.deepEqual(result,{kind:'success',value:workload==='single'?{...a,note}:{...b,title:title2}});
     count++;
    }
    if(record){
     const elapsedMs=performance.now()-start,samples=env.timing!.snapshot();assert.equal(samples.length,latency.length);assert(samples.every(s=>!s.rollback&&s.calls.length===(workload==='single'?1:2)));
     const stats=Object.fromEntries(['totalMs','beginMs','bodyMs','finishMs','sqlMs'].map(k=>[k,{p50:q(samples.map(s=>(s as any)[k]),.5),p95:q(samples.map(s=>(s as any)[k]),.95),p99:q(samples.map(s=>(s as any)[k]),.99),max:Math.max(...samples.map(s=>(s as any)[k]))}]));
     const slow=samples.filter(s=>s.totalMs>=10);
     const item={run:rep+1,target,mode,workload,rollbackPreflightVerified:true,pragmas:{...env.pragmas,journal,sqliteVersion,fullfsync,walAutoCheckpoint:env.db.query('PRAGMA wal_autocheckpoint').get()},count:latency.length,elapsedMs,throughput:latency.length*1000/elapsedMs,p95Ms:q(latency,.95),p99Ms:q(latency,.99),maxMs:Math.max(...latency),stats,slowCount:slow.length,slowStageTotals:slow.reduce((a,s)=>({beginMs:a.beginMs+s.beginMs,bodyMs:a.bodyMs+s.bodyMs,finishMs:a.finishMs+s.finishMs,sqlMs:a.sqlMs+s.sqlMs}),{beginMs:0,bodyMs:0,finishMs:0,sqlMs:0}),slowest:samples.slice().sort((a,b)=>b.totalMs-a.totalMs).slice(0,12),snapshot:env.snapshot()};
     results.push(item);writeFileSync(out+'/results.json',JSON.stringify({scope:'Follow-up journal diagnostic: rotated DELETE/FULL versus WAL/FULL on the same local filesystem, native Bun transaction wrapper unchanged. Five repetitions with 0.2s warmup /1.5s measurement; instrumentation and sequential local calls are not production capacity. WAL autocheckpoint recorded; latest rows must survive clean close/reopen (not a power-loss test).',results},null,2)+'\n');console.log(JSON.stringify({run:rep+1,target,mode,workload,count:item.count,p99:item.p99Ms,slowCount:item.slowCount,max:item.maxMs}));
    }
   }
  }
  finalSnapshot=env.snapshot();
 }finally{env.close();}
 const reopened=new Database(`${out}/${target}-${mode}-${rep}.sqlite`,{readonly:true});
 try{assert.deepEqual(reopened.query('SELECT * FROM entity_Item ORDER BY id').all(),finalSnapshot);}finally{reopened.close();}
}

writeFileSync(out+'/reopen.json',JSON.stringify({databases:20,allCleanReopensVerified:true,scope:'Clean reopen only; no simulated power loss.'},null,2)+'\n');
