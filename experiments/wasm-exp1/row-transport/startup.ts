import {spawn,execFileSync} from 'node:child_process';
import {mkdtempSync,rmSync,readFileSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {randomUUID} from 'node:crypto';
import assert from 'node:assert/strict';
const bun=execFileSync('which',['bun'],{encoding:'utf8'}).trim();
const spec=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8'));
const results:any[]=[];
for(let run=0;run<20;run++)for(const target of run%2?['candidate','bun']:['bun','candidate']){
 const dir=mkdtempSync(join(tmpdir(),'jadpo-row-startup-')),token=randomUUID(),start=performance.now();
 const proc=spawn(bun,['--no-install','--env-file=/dev/null',import.meta.dirname+'/startup-server.ts',target,dir+'/db.sqlite',token],{stdio:['ignore','pipe','pipe'],env:{...process.env,DATABASE_URL:undefined}});
 const exited=new Promise<number|null>((resolve,reject)=>{proc.on('error',reject);proc.on('exit',resolve)});
 let stderr='';proc.stderr.on('data',b=>stderr+=b);let buffer='';
 try{
  const ready:any=await new Promise((resolve,reject)=>{const timeout=setTimeout(()=>reject(Error('startup timeout '+stderr)),10000);proc.stdout.on('data',b=>{buffer+=b;if(buffer.includes('\n')){clearTimeout(timeout);try{resolve(JSON.parse(buffer.split('\n')[0]));}catch(e){reject(e);}}});exited.then(code=>{clearTimeout(timeout);reject(Error('early exit '+code+' '+stderr));},reject);});
  const readyMs=performance.now()-start,t=performance.now();const headers={'x-token':token};
  const response=await fetch(ready.url,{headers});assert.equal(response.status,200);assert.deepEqual(await response.json(),{kind:'success',value:spec.seeds.Item[0]});
  const firstHttpMs=performance.now()-t,processToFirstResponseMs=performance.now()-start;
  await fetch(ready.url+'stop',{headers});assert.equal(await exited,0);
  results.push({run:run+1,target,readyMs,firstHttpMs,processToFirstResponseMs,verified:true});
 }finally{if(proc.exitCode===null){proc.kill();await exited;}rmSync(dir,{recursive:true,force:true});}
}
writeFileSync('build/wasm-exp1/row-transport/startup.json',JSON.stringify({scope:'20 fresh processes per target, alternating order, real SQLite setup/seeding and first authenticated-harness HTTP read. No application preflight/warmup. Warm OS caches; local host, not Cloudflare cold starts. Process-to-first excludes shutdown/cleanup.',results},null,2)+'\n');
console.log(JSON.stringify(results));
