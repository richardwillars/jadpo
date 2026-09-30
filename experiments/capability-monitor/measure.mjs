import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {request,Agent} from 'node:http';
import os from 'node:os';

// Short attribution run. It deliberately does not replace the frozen campaign.
const root=resolve('.'),fixture=resolve('build/capability-host'),out=resolve('build/capability-monitor');
const seed=JSON.parse(readFileSync(fixture+'/seed.json','utf8'));
const targets=['native','raw-wasm','wasm','monitor'];
const workloads=['missing_auth','authorized_read','denied_pair'];
const repetitions=3,concurrency=12,block=128,minMeasureMs=300;
const [a1,,c1]=seed.notes.map(n=>n.id);
const q=(v,p)=>v[Math.min(v.length-1,Math.ceil(v.length*p)-1)];
const runId=Date.now(),results=[];let serial=0;
function ps(pid){
  const [time,rss]=execFileSync('/bin/ps',['-p',String(pid),'-o','time=,rss='],{encoding:'utf8'}).trim().split(/\s+/);
  const parts=time.split(':').map(Number);
  return {cpuMs:parts.reduce((v,n)=>v*60+n,0)*1000,rssBytes:Number(rss)*1024};
}
async function start(target){
  const dir=out+`/breakdown-${runId}-${++serial}-${target}`;mkdirSync(dir,{recursive:true});
  const path=dir+'/state.sqlite';copyFileSync(fixture+'/seed.sqlite',path);
  const token='LOCAL_SYNTHETIC_CONTROL';
  const command=target==='native'
    ? [fixture+'/target/release/jadpo-capability-host-native']
    : ['bun','--no-install','--env-file=/dev/null',root+(target==='raw-wasm'?'/experiments/auth-policy/bun-server.ts':target==='monitor'?'/experiments/capability-monitor/bun-server.ts':'/experiments/capability-host/bun-server.ts')];
  const started=performance.now();
  const child=spawn('/usr/bin/time',['-l',...command],{env:{...process.env,...seed.configuration,SQLITE_PATH:path,DATABASE_URL:'',TEST_CONTROL_TOKEN:token,EXPERIMENT_TARGET:target==='raw-wasm'?'wasm':target,JOURNAL:'WAL',PORT:'0'},stdio:['ignore','pipe','pipe']});
  let stderr='',stdout='';child.stderr.on('data',b=>stderr+=b);
  const exit=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',resolve)});
  const info=await new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>reject(Error('startup timeout '+stderr)),20000);
    child.stdout.on('data',b=>{stdout+=b;if(stdout.includes('\n')){clearTimeout(timer);try{resolve(JSON.parse(stdout.split('\n')[0]))}catch(e){reject(e)}}});
    exit.then(code=>{clearTimeout(timer);reject(Error(`startup exit ${code} ${stderr}`))},reject);
  });
  const agent=new Agent({keepAlive:true,maxSockets:concurrency});
  const send=(path,body,headers={})=>new Promise((resolve,reject)=>{
    const raw=JSON.stringify(body);const req=request(info.url+path.slice(1),{method:'POST',agent,headers:{'content-type':'application/json','content-length':Buffer.byteLength(raw),...headers}},res=>{
      const chunks=[];res.on('data',b=>chunks.push(b));res.on('error',reject);res.on('end',()=>{try{const body=JSON.parse(Buffer.concat(chunks).toString());assert.equal(res.headers['cache-control'],'no-store');assert.ok(res.headers['x-request-id']);resolve({status:res.statusCode,body})}catch(e){reject(e)}});
    });req.on('error',reject);req.setTimeout(20000,()=>req.destroy(Error('request timeout')));req.end(raw);
  });
  const control=async body=>{const r=await send('/__control',body,{'x-experiment-control':token});assert.equal(r.status,200);return r.body;};
  const call=(path,body,user='alice')=>send(path,body,user?{authorization:`Bearer ${seed.credentials[user]}`} :{});
  return {dir,info,started,send,control,call,stats:()=>ps(info.pid),stop:async()=>{agent.destroy();process.kill(info.pid,'SIGTERM');await exit;writeFileSync(dir+'/stderr.log',stderr);for(const secret of [...Object.values(seed.credentials),...Object.values(seed.configuration)])assert.ok(!stderr.includes(secret),'secret in runtime log');const cpu=stderr.match(/([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys/),rss=stderr.match(/(\d+)\s+maximum resident set size/);assert(cpu&&rss,'external process accounting missing: '+stderr);return {wallSeconds:+cpu[1],userSeconds:+cpu[2],systemSeconds:+cpu[3],peakRssBytes:+rss[1]};}};
}
function requestFor(server,workload){
  if(workload==='health')return ()=>server.send('/health',{});
  if(workload==='control_read')return async()=>{const body=await server.control({sql:'SELECT * FROM note WHERE id=?',params:[a1]});return {status:200,body};};
  if(workload==='missing_auth')return ()=>server.call('/notes/read',{id:a1},null);
  if(workload==='authorized_read')return ()=>server.call('/notes/read',{id:a1});
  return ()=>server.call('/notes/pair',{first:a1,second:c1,title:'first',second_title:'second'});
}
for(let rep=1;rep<=repetitions;rep++)for(const target of targets){
  const server=await start(target);
  try{
    for(const workload of workloads){
      const one=requestFor(server,workload);
      const expected=workload==='health'||workload==='control_read'?200:workload==='authorized_read'?200:workload==='denied_pair'?404:401;
      for(let i=0;i<32;i++){const r=await one();assert.equal(r.status,expected,`${target}/${workload}`);}
      await server.control({trace:true});
      const before=server.stats(),startCpu=process.cpuUsage(),phaseStart=performance.now();let activeMs=0,count=0,latencies=[];
      do{
        let next=0;const blockStart=performance.now();
        await Promise.all(Array.from({length:concurrency},async()=>{while(next++<block){const t=performance.now(),r=await one();assert.equal(r.status,expected,`${target}/${workload}`);latencies.push(performance.now()-t);count++;}}));
        activeMs+=performance.now()-blockStart;
        if(workload==='authorized_read'||workload==='denied_pair')await server.control({trace:true});
      }while(activeMs<minMeasureMs);
      const inclusiveMs=performance.now()-phaseStart,after=server.stats();
      latencies.sort((a,b)=>a-b);
      results.push({rep,target,workload,count,activeMs,inclusiveMs,rps:count*1000/activeMs,inclusiveRps:count*1000/inclusiveMs,p50Ms:q(latencies,.5),p95Ms:q(latencies,.95),p99Ms:q(latencies,.99),serverCpuMs:after.cpuMs-before.cpuMs,cpuUsPerRequest:(after.cpuMs-before.cpuMs)*1000/count,startRssBytes:before.rssBytes,endRssBytes:after.rssBytes});
      console.log(JSON.stringify({rep,target,workload,count,rps:count*1000/activeMs,p95Ms:q(latencies,.95),cpuUsPerRequest:(after.cpuMs-before.cpuMs)*1000/count}));
    }
  }finally{await server.stop();}
}
const report={scope:'monitor follow-up; all WASM Bun-hosted',started:new Date().toISOString(),machine:{platform:os.platform(),arch:os.arch(),cpu:os.cpus()[0].model,bun:execFileSync('bun',['--version'],{encoding:'utf8'}).trim(),node:process.version},protocol:{targets,workloads,repetitions,concurrency,block,minMeasureMs,trace:'drained after every timed block for application workloads'},results};
writeFileSync(out+'/breakdown.json',JSON.stringify(report,null,2)+'\n');

