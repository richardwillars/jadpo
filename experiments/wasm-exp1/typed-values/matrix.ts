import {environment,originalRows} from './environment.ts';
import {mkdirSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const out='build/wasm-exp1/typed-values/matrix';mkdirSync(out,{recursive:true});
const targets=['bun','previous','candidate'],envs:any={};
for(const t of targets)envs[t]=await environment(t,out+'/'+t+'.sqlite');
const results:any[]=[];
for(const size of [256,4096,16384,49152])for(const kind of ['ascii','escaped']){
 const note=kind==='ascii'?'x'.repeat(size):'é中😀"\\\n\t\0'.repeat(Math.floor(size/64));
 const expected={kind:'success',value:{...originalRows[0],note}},frameBytes=new TextEncoder().encode(JSON.stringify(expected)).length;
 assert(frameBytes<=65536);
 for(let run=0;run<5;run++)for(const target of [...targets.slice(run%3),...targets.slice(0,run%3)]){
  const e=envs[target];e.reset(note);let result:any;
  for(const record of [false,true]){
   let count=0;const samples:number[]=[];const cpu=process.cpuUsage(),start=performance.now(),end=start+(record?500:100);
   while(performance.now()<end){const t=performance.now();const value=await e.call('Item.read',originalRows[0].id);assert.deepEqual(value,expected);if(record)samples.push(performance.now()-t);count++;}
   const elapsed=performance.now()-start,usage=process.cpuUsage(cpu);samples.sort((a,b)=>a-b);
   if(record)result={run:run+1,target,size,kind,frameBytes,count,errors:0,throughput:count*1000/elapsed,p95Ms:samples[Math.floor(samples.length*.95)],cpuUs:(usage.user+usage.system)/count};
  }
  results.push(result);console.log(JSON.stringify(result));writeFileSync(out+'/results.json',JSON.stringify({scope:'Supplemental isolated matrix, 5 rotated pairs, 0.1s warmup/0.5s measurement. CPU includes harness verification. Escaped cases sized to fit the original JSON limit; compare within each cell, not equal bytes across kinds.',results},null,2)+'\n');
 }
}
for(const e of Object.values(envs) as any[])e.close();
