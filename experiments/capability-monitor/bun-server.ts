// Local monitor comparison server. All policy and SQL planning stay in the
// trusted Rust monitor; Bun only supplies HTTP, WebAssembly and SQLite drivers.
import {Database} from 'bun:sqlite';
import {Guest} from './monitor-driver.ts';
import {MonitorHost} from './monitor-host.ts';
import {readFileSync} from 'node:fs';

const path=Bun.env.SQLITE_PATH!;delete Bun.env.DATABASE_URL;
const journal=Bun.env.JOURNAL??'WAL';if(!['WAL','DELETE'].includes(journal))throw Error('journal');
const control=Bun.env.TEST_CONTROL_TOKEN!;if(!control||!path)throw Error('explicit local fixture required');
let trace:any[]=[];let recording=false;
const prepare=Database.prototype.prepare,exec=Database.prototype.exec;const configured=new Set<Database>();
function configure(db:Database){if(configured.has(db))return;configured.add(db);exec.call(db,`PRAGMA journal_mode=${journal}; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=0;`);}
Database.prototype.prepare=function(sql:string,...rest:any[]){configure(this);const statement=prepare.call(this,sql,...rest as any);return new Proxy(statement,{get(t,k){const v=Reflect.get(t,k,t);return typeof v!=='function'?v:(...args:any[])=>{if(recording&&['all','get','run'].includes(String(k)))trace.push({sql,parameters:args});return v.apply(t,args);};}});};
Database.prototype.exec=function(sql:string,...rest:any[]){configure(this);if(recording)trace.push({sql,parameters:rest});return exec.call(this,sql,...rest as any);};
const db=new Database(path,{strict:true});configure(db);
const contract=JSON.parse(readFileSync(import.meta.dir+'/../capability-host/build/contract.json','utf8'));
const configuration=Object.fromEntries([contract.auth.secretBinding,contract.auth.previousBinding].map(k=>[k,Bun.env[k]]));
const monitorModule=new WebAssembly.Module(readFileSync(import.meta.dir+'/monitor.wasm'));
const applicationModule=new WebAssembly.Module(readFileSync(import.meta.dir+'/application.wasm'));
const host=new MonitorHost(new Guest(monitorModule),()=>new Guest(applicationModule),db);
function scalarJson(value:any):void{if(typeof value==='string'){if(/[\uD800-\uDFFF]/u.test(value))throw Error('invalid Unicode scalar');}else if(value&&typeof value==='object')for(const [key,child] of Object.entries(value)){scalarJson(key);scalarJson(child);}}
async function handle(request:Request){
 const path=new URL(request.url).pathname,body=await request.text();
 if(Buffer.byteLength(body)>32768)return Response.json({error:{code:'request_too_large'}},{status:413});
 if(path==='/__control'&&request.headers.get('x-experiment-control')===control){const value=JSON.parse(body);if(value.trace){const result=trace;trace=[];return Response.json(result);}try{return Response.json(db.prepare(value.sql).all(...value.params));}catch{return Response.json({error:'control_sql'});}}
 if(path==='/health')return Response.json({ready:true});
 const frame={method:request.method,path,authorization:request.headers.get('authorization'),cookie:request.headers.get('cookie'),body,now:Date.now(),configuration};
 if(Buffer.byteLength(JSON.stringify(frame))>65536)return Response.json({error:{code:'internal_fault',message:'An internal error occurred.'}},{status:500});
 scalarJson(body);recording=true;
 try{const result=host.invoke(frame);if(result.kind!=='success')return Response.json({error:{code:'internal_fault',message:'An internal error occurred.'}},{status:500});return Response.json(result.value.body,{status:result.value.status});}
 finally{recording=false;}
}
async function transport(request:Request){const result=await handle(request);result.headers.set('cache-control','no-store');if(result.headers.has('x-request-id'))return result;const requestId=`req_${crypto.randomUUID()}`,body=await result.json() as any;if(body?.error&&typeof body.error==='object')body.error.request_id=requestId;return Response.json(body,{status:result.status,headers:{...Object.fromEntries(result.headers),'x-request-id':requestId}});}
let queue=Promise.resolve();
const server=Bun.serve({hostname:'127.0.0.1',port:Number(Bun.env.PORT??0),maxRequestBodySize:65536,fetch(request){const pending=queue.then(()=>transport(request));queue=pending.then(()=>{},()=>{});return pending;}});
const pragmas=[...configured].map(db=>Object.fromEntries(['journal_mode','synchronous','foreign_keys','busy_timeout','fullfsync','wal_autocheckpoint'].map(key=>[key,Object.values(prepare.call(db,`PRAGMA ${key}`).get() as object)[0]])));
console.log(JSON.stringify({url:server.url.href,pid:process.pid,target:'monitor',pragmas,sqliteVersion:(prepare.call(db,'SELECT sqlite_version() AS version').get() as any).version}));
