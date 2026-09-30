import {environment,originalRows} from './environment.ts';
import {writeFileSync,mkdirSync} from 'node:fs';
const out='build/wasm-exp1/read-path/'+(Bun.argv.includes('--selected')?'micro-selected':'micro');mkdirSync(out,{recursive:true});
const targets=Bun.argv.includes('--selected')?['bun','previous','candidate']:['bun','previous','egress','typed','reference','combined','compact','compact-reference','compact-combined'];
const envs:any={};for(const target of targets)envs[target]=await environment(target,out+'/'+target+'.sqlite');
const results:any[]=[];
for(const size of [256,16384])for(let run=0;run<5;run++)for(const target of [...targets.slice(run%targets.length),...targets.slice(0,run%targets.length)]){
 const e=envs[target];e.reset(size===16384);const input={id:originalRows[0].id,title:'fallback',note:''};input.note='x'.repeat(256-JSON.stringify(input).length);
 const operation=size===256?'probe':'Item.read',argument=size===256?input:originalRows[0].id;
 let count=0,errors=0;const samples:number[]=[];
 for(const record of [false,true]){const start=performance.now(),end=start+(record?1000:200);let batch=0;
 while(performance.now()<end){const t=performance.now(),r=await e.call(operation,argument);const ok=r.kind==='success'&&(size===256?r.value==='alpha':r.value.id===originalRows[0].id&&r.value.note==='x'.repeat(16384));if(!ok)throw Error('micro semantic mismatch');if(record){count++;samples.push(performance.now()-t);}if(++batch%256===0)await Bun.sleep(0);}
 if(record){samples.sort((a,b)=>a-b);results.push({run:run+1,target,size,requests:count,errors,throughput:count*1000/(performance.now()-start),p50Ms:samples[Math.floor(samples.length*.5)],p95Ms:samples[Math.floor(samples.length*.95)],p99Ms:samples[Math.floor(samples.length*.99)]});}
 }
 console.log(JSON.stringify(results.at(-1)));writeFileSync(out+'/results.json',JSON.stringify({scope:'Uninstrumented application boundary, small probe or 16KiB row read; separate optimisation variants, local file SQLite; 5 rotated runs, 0.2s warmup +1s timing. JSON HTTP boundary is separate.',results},null,2)+'\n');
}
for(const e of Object.values(envs) as any[])e.close();
