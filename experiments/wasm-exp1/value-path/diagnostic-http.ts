import {mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {randomUUID,createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {Readable} from 'node:stream';
import {Agent,request as httpRequest} from 'node:http';
const bunExecutable=execFileSync('which',['bun'],{encoding:'utf8'}).trim();
// Node's HTTP client avoids the repeatable Bun 1.2.20 fetch-control failure.
// Keep-alive connections are owned by this generator and explicitly closed.
const agent=new Agent({keepAlive:true,maxSockets:16});
function fetch(url:string,options:any):Promise<any>{return new Promise((resolve,reject)=>{
 const request=httpRequest(url,{method:options.method,headers:{...options.headers,'content-length':Buffer.byteLength(options.body)},agent},response=>{
  const chunks:Buffer[]=[];response.on('data',chunk=>chunks.push(chunk));response.on('error',reject);response.on('end',()=>{const text=Buffer.concat(chunks).toString();resolve({status:response.statusCode,ok:response.statusCode!>=200&&response.statusCode!<300,json:async()=>JSON.parse(text)});});
 });request.on('error',reject);request.setTimeout(10000,()=>request.destroy(Error('HTTP request timeout')));request.end(options.body);
});}
function launch(command:string[],options:any){const child=spawn(command[0],command.slice(1),{cwd:options.cwd,env:options.env,stdio:['ignore','pipe','pipe']});
 const exited=new Promise<number|null>((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);});
 return {stdout:Readable.toWeb(child.stdout!),stderr:Readable.toWeb(child.stderr!),exited,get exitCode(){return child.exitCode},kill:()=>child.kill()};
}
const root=resolve(import.meta.dirname,'../../..'),out=root+'/build/wasm-exp1/value-path/write-diagnostic';mkdirSync(out,{recursive:true});
const spec=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/acceptance.json','utf8'));
const [a,b,c]=spec.seeds.Item;
const smoke=process.argv.includes('--smoke');
const protocol={repetitions:3,concurrency:[16],warmupSeconds:smoke?.05:1,measureSeconds:smoke?.1:3,workloads:['write_single','write_pair'],order:'Rotate targets by repetition; reverse workload order on alternate repetitions; separate server per target/repetition/concurrency',sqlite:'On-disk SQLite default DELETE journal and FULL synchronous durability, verified per server',scope:'Closed-loop loopback HTTP; shared workstation; trusted fixture owner; no production authentication'};
const results:any[]=[],servers:any[]=[];
const q=(xs:number[],p:number)=>xs[Math.floor((xs.length-1)*p)]??null;
const hash=(p:string)=>createHash('sha256').update(readFileSync(p)).digest('hex');
const hashes={runner:hash(new URL(import.meta.url).pathname),server:hash(import.meta.dirname+'/diagnostic-server.ts'),environment:hash(import.meta.dirname+'/environment.ts'),previous:hash(root+'/experiments/wasm-exp1/boundary-http/compiler/build/application.wasm'),candidate:hash(import.meta.dirname+'/compiler/build/application.wasm'),bun:hash(root+'/build/wasm-exp1/baseline/generated/bun/target/app.ts')};
function save(){writeFileSync(out+(smoke?'/smoke.json':'/results.json'),JSON.stringify({protocol,hashes,versions:{clientNode:process.version,serverBun:execFileSync(bunExecutable,['--version'],{encoding:'utf8'}).trim(),platform:process.platform,arch:process.arch},servers,results},null,2)+'\n');}
for(const concurrency of protocol.concurrency)for(let rep=0;rep<protocol.repetitions;rep++){
 const base=['bun','candidate'];const offset=rep%base.length;const targets=[...base.slice(offset),...base.slice(0,offset)];
 for(const target of targets){
  const token=randomUUID(),db=out+`/${target}-${concurrency}-${rep}.sqlite`;
  const proc=launch([bunExecutable,'--no-install','--env-file=/dev/null',import.meta.dirname+'/diagnostic-server.ts',target,db,token],{cwd:root,stdout:'pipe',stderr:'pipe',env:{...process.env,DATABASE_URL:undefined}});
  let url='';let state:any;const stderr=new Response(proc.stderr).text();const reader=proc.stdout.getReader();
  try{
   const ready=await Promise.race([reader.read(),new Promise<never>((_,reject)=>setTimeout(()=>reject(Error('server ready timeout')),10000).unref())]);
   if(ready.done)throw Error('server exited: '+await stderr);
   state=JSON.parse(new TextDecoder().decode(ready.value).trim());url=state.url;servers.push({target,concurrency,run:rep+1,...state});save();
   assert.equal(state.pragmas.journal.journal_mode,'delete');assert.equal(state.pragmas.synchronous.synchronous,2);assert.equal(state.pragmas.foreignKeys.foreign_keys,1);
   const headers={'content-type':'application/json','x-experiment-token':token};
   const control=async(path:string,body:any={})=>{const r=await fetch(url+path,{method:'POST',headers,body:JSON.stringify(body)});if(!r.ok)throw Error('control '+path);return r.json() as any;};
   const workloads=target==='noop'?['read_small','read_large']:[...protocol.workloads];if(rep%2)workloads.reverse();
   for(const workload of workloads){
    const large=workload==='read_large';await control('reset',{large});
    let errors=0,count=0,firstError:any=null,clientLoopMaxMs=0,lastTick=performance.now();const tick=setInterval(()=>{const now=performance.now();clientLoopMaxMs=Math.max(clientLoopMaxMs,now-lastTick-10);lastTick=now;},10);const latency:number[]=[],markers=new Set<string>();
    const phase=async(seconds:number,record:boolean)=>{
     const end=performance.now()+seconds*1000;
     await Promise.all(Array.from({length:concurrency},async(_,lane)=>{let sequence=0;while(performance.now()<end){
      const marker=`${record?'m':'w'}${lane}-${sequence++}`;let operation:string,input:any;
      if(workload==='read_small'){operation='probe';input={id:a.id,title:'fallback',note:''};input.note='x'.repeat(256-JSON.stringify(input).length);}
      else if(large){operation='Item.read';input=a.id;}
      else if(workload==='write_single'){operation='Item.change';input=[a.id,{note:marker+'x'.repeat(256-marker.length)}];}
      else {operation='update_pair';input=[a.id,'one'+lane+'-'+(sequence%2),b.id,'two'+lane+'-'+(sequence%2)];}
      const body=JSON.stringify({operation,input}),started=performance.now();let ok=false;
      try{
       const response=await fetch(url+(target==='noop'?'noop':'call'),{method:'POST',headers,body});const result:any=await response.json();
       const wanted=workload==='read_small'?'alpha':large?{...a,note:'x'.repeat(16384)}:workload==='write_single'?{...a,note:input[1].note}:{...b,title:input[3]};
       assert.equal(response.status,200);assert.deepEqual(result,{kind:'success',value:wanted});ok=true;
       if(record&&workload==='write_single')markers.add(input[1].note);
      }catch(error){firstError??=JSON.stringify({message:String(error),code:(error as any)?.code,cause:String((error as any)?.cause)});}
      if(!ok&&!record)throw Error('warmup mismatch '+firstError+' sequence='+sequence+' workload='+workload);
      if(record){count++;if(!ok)errors++;latency.push(performance.now()-started);}
     }}));
    };
    await phase(protocol.warmupSeconds,false);await control('begin');clientLoopMaxMs=0;lastTick=performance.now();const cpu=process.cpuUsage(),started=performance.now();await phase(protocol.measureSeconds,true);const elapsedMs=performance.now()-started,clientCpu=process.cpuUsage(cpu);const stats=await control('stats');clearInterval(tick);
    assert.equal(stats.count,count,'server/client count');
    if(workload==='read_small'||large)assert.deepEqual(stats.snapshot,[large?{...a,note:'x'.repeat(16384)}:a,b,c]);
    else if(workload==='write_single'){assert(markers.has(stats.snapshot[0].note));assert.deepEqual({...stats.snapshot[0],note:a.note},a);assert.deepEqual(stats.snapshot.slice(1),[b,c]);}
    else {assert(/^one\d+-[01]$/.test(stats.snapshot[0].title));assert.equal(stats.snapshot[1].title,'two'+stats.snapshot[0].title.slice(3));assert.deepEqual({...stats.snapshot[0],title:a.title},a);assert.deepEqual({...stats.snapshot[1],title:b.title},b);assert.deepEqual(stats.snapshot[2],c);}
    latency.sort((x,y)=>x-y);const result={run:rep+1,target,concurrency,workload,count,errors,firstError,elapsedMs,throughput:(count-errors)*1000/elapsedMs,maxMs:latency.at(-1),clientLoopMaxMs,p50Ms:q(latency,.5),p95Ms:q(latency,.95),p99Ms:q(latency,.99),clientCpu,server:{handlerMaxMs:stats.handlerMaxMs,callMaxMs:stats.callMaxMs,loopMaxMs:stats.loopMaxMs,count:stats.count,cpu:stats.cpu,elapsedMs:stats.elapsedMs,handlerP50Ms:stats.handlerP50Ms,handlerP95Ms:stats.handlerP95Ms,handlerP99Ms:stats.handlerP99Ms},snapshotVerified:true};results.push(result);save();console.log(JSON.stringify(result));if(errors)throw Error('HTTP semantic mismatch');
   }
   await control('stop');await proc.exited;
  }catch(error){const exitBeforeCleanup=proc.exitCode;if(proc.exitCode===null){proc.kill();await proc.exited;}writeFileSync(out+'/failure.json',JSON.stringify({error:String(error),target,concurrency,run:rep+1,state,exitBeforeCleanup,exitAfterCleanup:proc.exitCode,stderr:await stderr},null,2));throw error;}finally{if(proc.exitCode===null){proc.kill();await proc.exited;}reader.releaseLock();}
 }
}
save();
agent.destroy();
