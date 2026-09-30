import {readFileSync,copyFileSync,mkdtempSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {performance} from 'node:perf_hooks';
import {Database} from 'bun:sqlite';
import {Guest} from './monitor-driver.ts';
import {MonitorHost} from './monitor-host.ts';

const seed=JSON.parse(readFileSync('build/capability-host/seed.json','utf8'));
const currentMonitor=readFileSync(new URL('./monitor.wasm',import.meta.url));
const currentApplication=readFileSync(new URL('./application.wasm',import.meta.url));
const previousMonitor=readFileSync(process.env.PREVIOUS_MONITOR_WASM??'/tmp/jadpo-monitor-previous-monitor.wasm');
const previousApplication=readFileSync(process.env.PREVIOUS_APPLICATION_WASM??'/tmp/jadpo-monitor-previous-application.wasm');
const iterations=Number(process.env.ITERATIONS??1000),warmup=Number(process.env.WARMUP??50),repetitions=Number(process.env.REPETITIONS??3);
const note=seed.notes[0];
const frame={method:'POST',path:'/notes/read',authorization:`Bearer ${seed.credentials.alice}`,cookie:null,body:JSON.stringify({id:note.id}),now:seed.now,configuration:seed.configuration};

function run(name,monitorBytes,applicationBytes){
 const samples=[];
 for(let rep=0;rep<repetitions;rep++){
  const dir=mkdtempSync(join(tmpdir(),'jadpo-monitor-in-process-'));const path=join(dir,'state.sqlite');copyFileSync('build/capability-host/seed.sqlite',path);
  const db=new Database(path),host=new MonitorHost(new Guest(monitorBytes),()=>new Guest(applicationBytes),db);
  for(let i=0;i<warmup;i++){const result=host.invoke(frame);if(result.kind!=='success'||result.value.body.id!==note.id)throw Error('warmup mismatch');}
  const cpuBefore=process.cpuUsage(),started=performance.now();
  for(let i=0;i<iterations;i++){const result=host.invoke(frame);if(result.kind!=='success'||result.value.body.id!==note.id||result.value.body.title!==note.title)throw Error('result mismatch');}
  const elapsedMs=performance.now()-started,cpu=process.cpuUsage(cpuBefore);
  samples.push({rep,elapsedMs,rps:iterations*1000/elapsedMs,cpuUsPerRequest:(cpu.user+cpu.system)/iterations});
  db.close();rmSync(dir,{recursive:true,force:true});
 }
 return {name,iterations,warmup,repetitions,samples};
}
const order=process.env.ORDER==='current-first' ? [['current',currentMonitor,currentApplication],['previous',previousMonitor,previousApplication]] : [['previous',previousMonitor,previousApplication],['current',currentMonitor,currentApplication]];
const results={scope:'in-process monitor path; Bun SQLite, no HTTP socket',order:process.env.ORDER==='current-first'?'current-first':'previous-first'};
for(const [name,monitor,application] of order)results[name]=run(name,monitor,application);
console.log(JSON.stringify(results));
