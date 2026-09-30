// Local comparison server: generated Bun HTTP target or shared Rust in Bun-hosted WASM.
import {Database} from 'bun:sqlite';
import {Guest} from './wasm-driver.ts';
import {AuthorityHost,authorityGuest} from './authority-host.ts';
import {createHash} from 'node:crypto';
import {sqliteHost} from './sqlite-host.ts';
import {readFileSync} from 'node:fs';
import {withBudgets} from '../native-conformance/bun-environment.ts';
const target=Bun.env.EXPERIMENT_TARGET??'bun';
const path=Bun.env.SQLITE_PATH!;delete Bun.env.DATABASE_URL;
const journal=Bun.env.JOURNAL??'WAL';if(!['WAL','DELETE'].includes(journal))throw Error('journal');
const control=Bun.env.TEST_CONTROL_TOKEN!;if(!control||!path)throw Error('explicit local fixture required');
let trace:any[]=[];let recording=false;
const prepare=Database.prototype.prepare,exec=Database.prototype.exec;const configured=new Set<Database>();
function configure(db:Database){if(configured.has(db))return;configured.add(db);exec.call(db,`PRAGMA journal_mode=${journal}; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=0;`);}
Database.prototype.prepare=function(sql:string,...rest:any[]){configure(this);const statement=prepare.call(this,sql,...rest as any);return new Proxy(statement,{get(t,k){const v=Reflect.get(t,k,t);return typeof v!=='function'?v:(...args:any[])=>{if(recording&&['all','get','run'].includes(String(k)))trace.push({sql,parameters:args});return v.apply(t,args);};}});};
Database.prototype.exec=function(sql:string,...rest:any[]){configure(this);if(recording)trace.push({sql,parameters:rest});return exec.call(this,sql,...rest as any);};
const db=new Database(path,{strict:true});configure(db);
const contract=JSON.parse(readFileSync(import.meta.dir+'/build/contract.json','utf8'));
const configuration=Object.fromEntries([contract.auth.secretBinding,contract.auth.previousBinding].map(k=>[k,Bun.env[k]]));
let app:any,guest:AuthorityHost;
if(target==='bun'){
 app=await import('./build/projected/bun/target/app.ts');
 const {persistence}=await import('./build/projected/bun/target/persistence.ts');
 // Preserve generated transaction/savepoint semantics, checking budgets before commit.
 const guarded=withBudgets({...persistence});
 for(const key of Object.keys(persistence))if(typeof persistence[key]==='function')persistence[key]=guarded[key];
 await app.initializeApplication(configuration);
}else {
 const lock=JSON.parse(readFileSync(import.meta.dir+'/build/trust.json','utf8'));
 if(createHash('sha256').update(readFileSync(import.meta.dir+'/build/contract.json')).digest('hex')!==lock.contract)throw Error('unapproved contract');
 const module=new WebAssembly.Module(readFileSync(import.meta.dir+'/build/application.wasm'));
 guest=new AuthorityHost(authorityGuest(readFileSync(import.meta.dir+'/build/authority.wasm'),lock.authority),()=>new Guest(module),db);
}
const host=sqliteHost(db);
// serde_json represents Unicode scalar strings. Reject lone UTF-16 surrogates
// at the same post-authentication JSON boundary in the Bun comparison wrapper.
function scalarJson(value:any):void {
 if(typeof value==='string'){if(/[\uD800-\uDFFF]/u.test(value))throw Error('invalid Unicode scalar');}
 else if(value&&typeof value==='object')for(const [key,child] of Object.entries(value)){scalarJson(key);scalarJson(child);}
}
async function handle(request:Request){
 const path=new URL(request.url).pathname;
 const body=await request.text();
 if(Buffer.byteLength(body)>32768)return Response.json({error:{code:'request_too_large'}},{status:413});
 if(path==='/__control'&&request.headers.get('x-experiment-control')===control){
  const value=JSON.parse(body);if(value.trace){const result=trace;trace=[];return Response.json(result);}
  try{return Response.json(db.prepare(value.sql).all(...value.params));}catch{return Response.json({error:'control_sql'});}
 }
 if(path==='/health')return Response.json({ready:true});
 const frame={method:request.method,path,authorization:request.headers.get('authorization'),cookie:request.headers.get('cookie'),body,now:Date.now(),configuration};
 // Apply the same bridge-frame cap to the comparison host, including escaped JSON.
 if(Buffer.byteLength(JSON.stringify(frame))>65536)return Response.json({error:{code:'internal_fault',message:'An internal error occurred.'}},{status:500});
 recording=true;
 try{
  if(target==='bun'){
   const input=new Request(request.url,{method:request.method,headers:request.headers,body});
   const parse=input.json.bind(input);input.json=async()=>{const value=await parse();scalarJson(value);return value;};
   return await app.handleRequest(input);
  }
  const result=guest!.invoke(frame);
  if(result.kind!=='success')return Response.json({error:{code:'internal_fault',message:'An internal error occurred.'}},{status:500});
  return Response.json(result.value.body,{status:result.value.status});
 }finally{recording=false;host.recover();}
}
// The native experiment also serializes requests on one connection. Auth still
// resolves live authority on every request; no identity/role cache is introduced.
async function transport(request:Request){
 const result=await handle(request);result.headers.set('cache-control','no-store');
 if(result.headers.has('x-request-id'))return result;
 const requestId=`req_${crypto.randomUUID()}`,body=await result.json() as any;
 if(body?.error&&typeof body.error==='object')body.error.request_id=requestId;
 return Response.json(body,{status:result.status,headers:{...Object.fromEntries(result.headers),'x-request-id':requestId}});
}
let queue=Promise.resolve();
const server=Bun.serve({hostname:'127.0.0.1',port:Number(Bun.env.PORT??0),maxRequestBodySize:65536,fetch(request){const pending=queue.then(()=>transport(request));queue=pending.then(()=>{},()=>{});return pending;}});
const pragmas=[...configured].map(db=>Object.fromEntries(['journal_mode','synchronous','foreign_keys','busy_timeout','fullfsync','wal_autocheckpoint'].map(key=>[key,Object.values(prepare.call(db,`PRAGMA ${key}`).get() as object)[0]])));
console.log(JSON.stringify({url:server.url.href,pid:process.pid,target,pragmas,sqliteVersion:(prepare.call(db,'SELECT sqlite_version() AS version').get() as any).version}));
