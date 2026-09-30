// Existing generated Bun target and its established trusted entry adapter.
import {Database} from 'bun:sqlite';
import assert from 'node:assert/strict';
const [dbPath,token,journal]=Bun.argv.slice(2);assert(['WAL','DELETE'].includes(journal));
const connections=new Set<Database>(),originalPrepare=Database.prototype.prepare,originalExec=Database.prototype.exec;
const configure=(db:Database)=>{if(connections.has(db))return;connections.add(db);originalExec.call(db,`PRAGMA journal_mode=${journal}; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA fullfsync=OFF; PRAGMA wal_autocheckpoint=1000; PRAGMA busy_timeout=0;`);};
Database.prototype.prepare=function(...args:any[]){configure(this);return originalPrepare.apply(this,args as any);};
Database.prototype.exec=function(...args:any[]){configure(this);return originalExec.apply(this,args as any);};
const {environment,originalRows,spec}=await import('../wasm-exp1/read-path/environment.ts');
const env=await environment('bun',dbPath);env.reset();
Database.prototype.prepare=originalPrepare;Database.prototype.exec=originalExec;
const pragmas=(db:Database)=>Object.fromEntries(['journal_mode','synchronous','foreign_keys','fullfsync','wal_autocheckpoint','busy_timeout','page_size'].map(k=>[k,(db.query(`PRAGMA ${k}`).get() as any)[k==='busy_timeout'?'timeout':k]]));
const allPragmas=[...connections].map(pragmas);for(const p of allPragmas){assert.equal(p.journal_mode,journal.toLowerCase());assert.equal(p.synchronous,2);}
let noopRow=structuredClone(originalRows[0]);
let count=0,cpu=process.cpuUsage(),start=performance.now(),trace:any[]=[],tracing=false;
// Diagnostic SQL traces are enabled only for correctness runs, never timed cells.
if(process.env.NATIVE_TRACE==='1'){
 Database.prototype.prepare=function(sql:string,...rest:any[]){const statement=originalPrepare.call(this,sql,...rest);return new Proxy(statement,{get(t,k){const v=Reflect.get(t,k,t);return typeof v!=='function'?v:(...args:any[])=>{if(tracing&&['all','get','run'].includes(String(k)))trace.push({sql,parameters:args});return v.apply(t,args);};}});};
 Database.prototype.exec=function(sql:string,...rest:any[]){if(tracing)trace.push({sql,parameters:rest});return originalExec.call(this,sql,...rest);};
}
const call=async(body:any)=>{const input=body.input;if(Buffer.byteLength(JSON.stringify(input)??'null')>65536)return {kind:'invalid'};const r=await env.call(body.operation,input,body.principal??'owner');if(Buffer.byteLength(JSON.stringify(r))>65536)return {kind:'internal'};return r;};
const server=Bun.serve({hostname:'127.0.0.1',port:0,maxRequestBodySize:65536,async fetch(req){
 if(req.headers.get('x-experiment-token')!==token)return new Response('',{status:404});
 const path=new URL(req.url).pathname;let result:any;
 try {const body=await req.json() as any;
 if(path==='/begin'){count=0;cpu=process.cpuUsage();start=performance.now();result={ok:true};}
 else if(path==='/stats')result={count,cpu:process.cpuUsage(cpu),elapsedMs:performance.now()-start,maxRssBytes:process.resourceUsage().maxRSS*(process.platform==='darwin'?1:1024),snapshot:env.snapshot(),trace};
 else if(path==='/reset'){result={rows:env.reset(Object.hasOwn(body,'note')?body.note:false)};trace=[];noopRow=structuredClone(result.rows[0]);}
 else if(path==='/snapshot')result=env.snapshot();
 else if(path==='/mutate'){env.db.exec(body.sql.replaceAll('entity_Item','item'));result={ok:true};}
 else if(path==='/trace'){tracing=body.enabled;trace=[];result={ok:true};}
 else if(path==='/noop'){count++;result={kind:'success',value:body.operation==='probe'?'alpha':{...noopRow}};}
 else if(path==='/micro'){const times:number[]=[],n=body.count;const c=process.cpuUsage(),t=performance.now();for(let i=0;i<n;i++){const s=performance.now();assert.deepEqual(await call(body),body.expected);times.push((performance.now()-s)*1000);}const elapsedMs=performance.now()-t,u=process.cpuUsage(c);times.sort((a,b)=>a-b);result={count:n,elapsedMs,throughput:n*1000/elapsedMs,p50Us:times[Math.floor((n-1)*.5)],p95Us:times[Math.floor((n-1)*.95)],p99Us:times[Math.floor((n-1)*.99)],maxUs:times.at(-1),cpuUs:u.user+u.system};}
 else {count++;result=await call(body);}
 } catch {result={kind:'invalid'};}
 return Response.json(result,{status:result.kind==='invalid'?400:result.kind==='domain'?422:result.kind==='internal'?500:200});
}});
console.log(JSON.stringify({url:server.url.href,pid:process.pid,pragmas:{...allPragmas[0],sqlite_version:(env.db.query('SELECT sqlite_version() AS version').get() as any).version,compile_options:env.db.query('PRAGMA compile_options').all()},allConnections:allPragmas}));
