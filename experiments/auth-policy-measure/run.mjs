import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,copyFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {request,Agent} from 'node:http';
import {gzipSync} from 'node:zlib';
import os from 'node:os';
const root=resolve('.'),out=resolve('build/auth-policy-measure'),fixture=resolve('build/auth-policy');
const seed=JSON.parse(readFileSync(fixture+'/seed.json','utf8'));
assert(Date.now()-seed.now<3000000,'Refresh synthetic credentials before measuring');
execFileSync('python3',['experiments/auth-policy-measure/freeze.py']);
mkdirSync(out,{recursive:true});
const smoke=process.argv.includes('--smoke'),startup=process.argv.includes('--startup');
const mode=startup?'startup':smoke?'smoke':'http',repetitions=startup?10:smoke?1:3;
const report={mode,machine:{platform:os.platform(),arch:os.arch(),release:os.release(),cpu:os.cpus()[0].model,cores:os.cpus().length,totalMemoryBytes:os.totalmem(),bun:execFileSync('bun',['--version'],{encoding:'utf8'}).trim(),node:process.version},started:new Date().toISOString(),protocol:{repetitions,concurrency:[1,12],block:128,warmups:32,minMeasureMs:smoke?100:1000,cpuResolutionMs:10,trace:'drained outside request timing after every block',cpu:'ps delta includes trace drains; process totals from time -l'},results:[]};
const save=()=>writeFileSync(out+'/'+mode+'.json',JSON.stringify(report,null,2)+'\n');
function ps(pid){const [time,rss]=execFileSync('/bin/ps',['-p',String(pid),'-o','time=,rss='],{encoding:'utf8'}).trim().split(/\s+/);const parts=time.split(':').map(Number);return {cpuMs:parts.reduce((v,n)=>v*60+n,0)*1000,rssBytes:Number(rss)*1024};}
const q=(v,p)=>v[Math.min(v.length-1,Math.ceil(v.length*p)-1)];
const runId=Date.now();let serial=0;
async function start(target,journal){
 const dir=out+`/${mode}-${runId}-${++serial}-${target}-${journal}`;mkdirSync(dir,{recursive:true});
 const path=dir+'/state.sqlite';copyFileSync(fixture+'/seed.sqlite',path);
 const token='LOCAL_SYNTHETIC_CONTROL';
 const command=target==='native'?[fixture+'/target/release/jadpo-auth-policy-native']:['bun','--no-install','--env-file=/dev/null',root+'/experiments/auth-policy/bun-server.ts'];
 const started=performance.now();
 const child=spawn('/usr/bin/time',['-l',...command],{env:{...process.env,...seed.configuration,SQLITE_PATH:path,DATABASE_URL:'',TEST_CONTROL_TOKEN:token,EXPERIMENT_TARGET:target,JOURNAL:journal,PORT:'0'},stdio:['ignore','pipe','pipe']});
 let stderr='',stdout='';child.stderr.on('data',b=>stderr+=b);
 const exit=new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',resolve)});
 let info;try{info=await new Promise((resolve,reject)=>{
  const timer=setTimeout(()=>reject(Error('startup timeout '+stderr)),20000);
  child.stdout.on('data',b=>{stdout+=b;if(stdout.includes('\n')){clearTimeout(timer);try{resolve(JSON.parse(stdout.split('\n')[0]))}catch(e){reject(e)}}});
  exit.then(code=>{clearTimeout(timer);reject(Error(`startup exit ${code} ${stderr}`))},reject);
 });}catch(e){child.kill();throw e;}
 const readyMs=performance.now()-started;
 for(const p of info.pragmas)for(const [key,value] of Object.entries({journal_mode:journal.toLowerCase(),synchronous:2,foreign_keys:1,busy_timeout:0,fullfsync:0,wal_autocheckpoint:1000}))assert.equal(p[key],value);
 const agent=new Agent({keepAlive:true,maxSockets:12});
 const send=(path,body,headers={})=>new Promise((resolve,reject)=>{
  const raw=JSON.stringify(body);const req=request(info.url+path.slice(1),{method:'POST',agent,headers:{'content-type':'application/json','content-length':Buffer.byteLength(raw),...headers}},res=>{
   const chunks=[];res.on('data',b=>chunks.push(b));res.on('error',reject);res.on('end',()=>{try{const body=JSON.parse(Buffer.concat(chunks).toString());assert.equal(res.headers['cache-control'],'no-store');assert.ok(res.headers['x-request-id']);if(body?.error){assert.equal(body.error.request_id,res.headers['x-request-id']);delete body.error.request_id;}resolve({status:res.statusCode,body})}catch(e){reject(e)}});
  });req.on('error',reject);req.setTimeout(20000,()=>req.destroy(Error('request timeout')));req.end(raw);
 });
 const control=async body=>{const r=await send('/__control',body,{'x-experiment-control':token});assert.equal(r.status,200);assert.equal(r.body?.error,undefined);return r.body;};
 const sql=(sql,params=[])=>control({sql,params});
 const call=(path,body,user='alice')=>send(path,body,user?{authorization:`Bearer ${seed.credentials[user]}`} :{});
 return {info,dir,started,readyMs,call,sql,control,stats:()=>ps(info.pid),stop:async()=>{agent.destroy();process.kill(info.pid,'SIGTERM');await exit;writeFileSync(dir+'/stderr.log',stderr);for(const secret of [...Object.values(seed.credentials),...Object.values(seed.configuration)])assert.ok(!stderr.includes(secret),'secret in runtime log');const cpu=stderr.match(/([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys/),rss=stderr.match(/(\d+)\s+maximum resident set size/);assert(cpu&&rss,'external process accounting missing: '+stderr);return {wallSeconds:+cpu[1],userSeconds:+cpu[2],systemSeconds:+cpu[3],peakRssBytes:+rss[1]};}};
}
const [a1,a2,c1]=seed.notes.map(n=>n.id);
const allWork=['read_small','read_large_public','read_large_private','write_single','write_pair','denied_pair','missing_auth'];
let active;
try{
 for(let rep=0;rep<repetitions;rep++){
  const order=['bun','native','wasm'];order.push(...order.splice(0,rep%3));
  for(const journal of startup||smoke?['WAL']:['WAL','DELETE'])for(const concurrency of startup?[1]:[1,12])for(const workload of startup?['startup']:rep%2?[...allWork].reverse():allWork)for(const target of order){
   const s=active=await start(target,journal);let result={evidenceDir:s.dir.slice(root.length+1),target,journal,concurrency,workload,rep:rep+1,info:s.info,readyMs:s.readyMs};
   try{
    if(startup){const t=performance.now();const r=await s.call('/notes/read',{id:a1});assert.deepEqual(r,{status:200,body:{id:a1,title:'Alice first'}});Object.assign(result,{firstHttpMs:performance.now()-t,spawnToFirstMs:performance.now()-s.started,...s.stats()});}
    else{
     const large=workload.includes('large'),note=large?'x'.repeat(16384):seed.notes[0].private_note;
     if(large)await s.sql('UPDATE note SET private_note=? WHERE id=?',[note,a1]);
     const before=await s.sql('SELECT * FROM note ORDER BY id');assert.deepEqual(before,seed.notes.map((row,i)=>large&&i===0?{...row,private_note:note}:row));
     let sequence=0;
     async function one(){
      const n=sequence++,start=performance.now();let r;
      if(workload==='write_single'){
       const title=`single-${n}`;r=await s.call('/notes/rename',{id:a1,title});assert.deepEqual(r,{status:200,body:{id:a1,title}});
      }else if(workload==='write_pair'||workload==='denied_pair'){
       const title=`first-${n}`,second_title=`second-${n}`,denied=workload==='denied_pair';r=await s.call('/notes/pair',{first:a1,second:denied?c1:a2,title,second_title});
       if(denied){assert.deepEqual(r,{status:404,body:{error:{code:'note_missing',message:'Request failed.'}}});}
       else assert.deepEqual(r,{status:200,body:{id:a2,title:second_title}});
      }else if(workload==='missing_auth'){
       r=await s.call('/notes/read',{id:a1},null);assert.equal(r.status,401);assert.equal(r.body.error.code,'authentication_required');
      }else{
       const privateRead=workload==='read_large_private';r=await s.call(privateRead?'/notes/private':'/notes/read',{id:a1});assert.deepEqual(r,{status:200,body:{id:a1,...(privateRead?{private_note:note}:{title:'Alice first'})}});
      }
      return performance.now()-start;
     }
     for(let i=0;i<32;i++)await one();await s.control({trace:true});
     const startStats=s.stats(),clientStart=process.cpuUsage(),phaseStart=performance.now();let activeMs=0,latencies=[],statements=0,blocks=0,exampleTrace;
     do{
      let next=0;const blockStart=performance.now();
      await Promise.all(Array.from({length:concurrency},async()=>{while(next++<128)latencies.push(await one());}));
      activeMs+=performance.now()-blockStart;blocks++;
      const trace=await s.control({trace:true});statements+=trace.length;exampleTrace??=trace.slice(0,20);
     }while(activeMs<(smoke?100:1000));
     const inclusiveMs=performance.now()-phaseStart,clientCpu=process.cpuUsage(clientStart),endStats=s.stats();
     const after=await s.sql('SELECT * FROM note ORDER BY id');
     if(workload==='write_single'){
      assert.match(after[0].title,/^single-\d+$/);assert.deepEqual(after.map((n,i)=>i===0?{...n,title:before[0].title}:n),before);
     }else if(workload==='write_pair'){
      assert.match(after[0].title,/^first-\d+$/);assert.equal(after[1].title,'second-'+after[0].title.slice(6));assert.deepEqual(after.map((n,i)=>i<2?{...n,title:before[i].title}:n),before);
     }else assert.deepEqual(after,before);
     const count=latencies.length,expected=workload==='missing_auth'?0:workload.startsWith('read')?4:workload==='write_single'?(target==='bun'?7:6):(target==='bun'?13:7);
     assert.equal(statements,count*expected,'SQL work changed');
     writeFileSync(s.dir+'/latencies.json.gz',gzipSync(JSON.stringify(latencies)));
     latencies.sort((a,b)=>a-b);
     Object.assign(result,{count,errors:0,blocks,activeMs,inclusiveMs,rps:count*1000/activeMs,inclusiveRps:count*1000/inclusiveMs,p50Ms:q(latencies,.5),p95Ms:q(latencies,.95),p99Ms:q(latencies,.99),maxMs:latencies.at(-1),serverCpuMs:endStats.cpuMs-startStats.cpuMs,cpuUsPerRequest:(endStats.cpuMs-startStats.cpuMs)*1000/count,clientCpu,startRssBytes:startStats.rssBytes,endRssBytes:endStats.rssBytes,statements,statementsPerRequest:expected,exampleTrace,snapshotVerified:true});
    }
   }finally{result.processAccounting=await s.stop();active=null;}
   report.results.push(result);save();console.log(JSON.stringify({rep:rep+1,target,journal,concurrency,workload,count:result.count,rps:result.rps,p95Ms:result.p95Ms}));
  }
 }
 report.completed=new Date().toISOString();save();execFileSync('python3',['experiments/auth-policy-measure/freeze.py']);
}catch(e){report.failure=String(e);save();throw e;}finally{if(active)await active.stop();}
