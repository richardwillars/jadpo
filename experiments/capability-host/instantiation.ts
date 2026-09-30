// Isolated diagnostic only: the reused-instance case is NOT a safe guest-host mode.
import {Guest} from './wasm-driver.ts';
import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const module=new WebAssembly.Module(readFileSync(import.meta.dir+'/build/application.wasm'));
const seed=JSON.parse(readFileSync('build/capability-host/seed.json','utf8'));
const route=JSON.parse(readFileSync(import.meta.dir+'/build/contract.json','utf8')).routes.find((r:any)=>r.path==='/notes/read');
const results=[];
for(let rep=0;rep<3;rep++){
 const order=['instantiate_only','fresh_first_effect','reused_first_effect'];order.push(...order.splice(0,rep));
 for(const mode of order){
  const reused=new Guest(module);
  const one=(i:number)=>{
   const g=mode==='reused_first_effect'?reused:new Guest(module);
   if(mode==='instantiate_only'){assert.equal(g.exports.result_len(),0);return;}
   const p=g.start(route.operation,{scope:i+1,input:{id:seed.notes[0].id}});
   assert.equal(p.kind,'pending');assert.equal(p.capability,'entity.read');assert.equal(p.requestId,i+1);g.cancel();
  };
  for(let i=0;i<100;i++)one(i);
  const cpu=process.cpuUsage(),start=performance.now();for(let i=0;i<2000;i++)one(i);
  const elapsedMs=performance.now()-start,delta=process.cpuUsage(cpu);
  results.push({rep:rep+1,mode,count:2000,elapsedMs,meanUs:elapsedMs*1000/2000,cpuUsPerIteration:(delta.user+delta.system)/2000,rssBytes:process.memoryUsage().rss});
 }
}
writeFileSync('build/capability-host/instantiation.json',JSON.stringify({scope:'Diagnostic: precompiled module, no database/auth/HTTP; unsafe cross-request reuse is only a cost control',results},null,2)+'\n');
console.log(JSON.stringify(results));
