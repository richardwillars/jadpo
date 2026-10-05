import {readFileSync,copyFileSync,mkdtempSync,rmSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {performance} from 'node:perf_hooks';
import {Database} from 'bun:sqlite';
import {Guest} from './monitor-driver.ts';
import {MonitorHost} from './monitor-host.ts';

const seed=JSON.parse(readFileSync('build/capability-host/seed.json','utf8'));
const monitorBytes=readFileSync(new URL('./monitor.wasm',import.meta.url));
const applicationBytes=readFileSync(new URL('./application.wasm',import.meta.url));
const iterations=Number(process.env.ITERATIONS??1000);
const warmup=Number(process.env.WARMUP??50);
const repetitions=Number(process.env.REPETITIONS??3);
const note=seed.notes[0];
const frame={method:'POST',path:'/notes/read',authorization:`Bearer ${seed.credentials.alice}`,cookie:null,body:JSON.stringify({id:note.id}),now:seed.now,configuration:seed.configuration};

function run(name,cacheStatements){
 const samples=[];
 for(let rep=0;rep<repetitions;rep++){
  const dir=mkdtempSync(join(tmpdir(),'jadpo-monitor-sqlite-cache-'));
  const path=join(dir,'state.sqlite');
  copyFileSync('build/capability-host/seed.sqlite',path);
  const db=new Database(path);
  const host=new MonitorHost(new Guest(monitorBytes),()=>new Guest(applicationBytes),db,cacheStatements);
  for(let i=0;i<warmup;i++){
   const result=host.invoke(frame);
   if(result.kind!=='success'||result.value.body.id!==note.id||result.value.body.title!==note.title)throw Error(`${name} warmup mismatch`);
  }
  const cpuBefore=process.cpuUsage();
  const started=performance.now();
  for(let i=0;i<iterations;i++){
   const result=host.invoke(frame);
   if(result.kind!=='success'||result.value.body.id!==note.id||result.value.body.title!==note.title)throw Error(`${name} result mismatch`);
  }
  const elapsedMs=performance.now()-started;
  const cpu=process.cpuUsage(cpuBefore);
  samples.push({rep,elapsedMs,rps:iterations*1000/elapsedMs,cpuUsPerRequest:(cpu.user+cpu.system)/iterations});
  db.close();
  rmSync(dir,{recursive:true,force:true});
 }
 return {name,cacheStatements,iterations,warmup,repetitions,samples};
}

const currentFirst=process.env.ORDER==='cached-first';
const order=currentFirst
 ? [['cached',true],['uncached',false]]
 : [['uncached',false],['cached',true]];
const results={scope:'in-process monitor path; Bun SQLite, no HTTP socket',order:currentFirst?'cached-first':'uncached-first'};
for(const [name,cache] of order)results[name]=run(name,cache);
const encoded=JSON.stringify(results);
if(process.env.OUTPUT)writeFileSync(process.env.OUTPUT,encoded+'\n');
console.log(encoded);
