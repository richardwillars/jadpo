import {environment,originalRows,spec} from './environment.ts';
import assert from 'node:assert/strict';
const [target,dbPath,token]=Bun.argv.slice(2);if(!token)throw Error('private harness token');
const env=await environment(target,dbPath,process.env.TYPED_SQL_TIMING==='1'&&target!=='bun');env.db.exec('PRAGMA journal_mode = WAL');env.pragmas.journal=env.db.query('PRAGMA journal_mode').get();const a=originalRows[0],b=originalRows[1];
const preflight:any[]=[];
async function verify(name:string,operation:string,input:any,expected:any,principal='owner'){
 env.reset();const before=env.snapshot();const actual=await env.call(operation,input,principal);assert.deepEqual(actual,expected,name);assert.deepEqual(env.snapshot(),before,name+' changed storage');preflight.push({name,status:'pass'});
}
await verify('owner','Item.read',a.id,{kind:'success',value:a});
await verify('outsider','Item.read',a.id,{kind:'domain',failure:'ItemMissing'},'other');
await verify('invalid','probe',{id:a.id,title:'x'},{kind:'invalid'});
await verify('missing','Item.read',spec.seeds.missing_item_id,{kind:'domain',failure:'ItemMissing'});
await verify('empty patch','Item.change',[a.id,{}],{kind:'domain',failure:'EmptyPatch'});
await verify('conflict','Item.rename',[a.id,b.title],{kind:'domain',failure:'ItemConflict'});
await verify('atomic rollback','update_pair',[a.id,'first',b.id,'charlie'],{kind:'domain',failure:'ItemConflict'});
await verify('outsider write','Item.change',[a.id,{note:'forbidden'}],{kind:'domain',failure:'ItemMissing'},'other');
env.reset();
let samples:number[]=[],count=0,startCpu=process.cpuUsage(),startTime=performance.now();
const quantile=(xs:number[],p:number)=>xs[Math.floor((xs.length-1)*p)]??null;
const server=Bun.serve({hostname:'127.0.0.1',port:0,maxRequestBodySize:65536,async fetch(request){
 if(request.headers.get('x-experiment-token')!==token)return new Response('Not found',{status:404});
 const path=new URL(request.url).pathname;
 if(path==='/begin'){env.timing?.reset();samples=[];count=0;startCpu=process.cpuUsage();startTime=performance.now();return Response.json({ok:true});}
 if(path==='/stats'){const cpu=process.cpuUsage(startCpu),elapsed=performance.now()-startTime;const sorted=samples.sort((a,b)=>a-b);return Response.json({count,cpu,elapsedMs:elapsed,handlerP50Ms:quantile(sorted,.5),handlerP95Ms:quantile(sorted,.95),handlerP99Ms:quantile(sorted,.99),snapshot:env.snapshot(),sqlTiming:env.timing?.summary()});}
 if(path==='/reset'){const {large}=await request.json() as any;return Response.json({rows:env.reset(!!large)});}
 if(path==='/stop'){setTimeout(()=>{server.stop(true);env.close();process.exit(0)},10);return Response.json({ok:true});}
 const start=performance.now();let response:Response;
 try{
  const {operation,input}=await request.json() as any;
  const result=path==='/noop'?{kind:'success',value:operation==='Item.read'?{...a,note:'x'.repeat(16384)}:'alpha'}:await env.call(operation,input);
  response=Response.json(result,{status:result.kind==='success'?200:result.kind==='invalid'?400:result.kind==='domain'?422:500});
 }catch{response=Response.json({kind:'invalid'},{status:400});}
 samples.push(performance.now()-start);count++;return response;
}});
console.log(JSON.stringify({ready:true,url:server.url.href,target,pragmas:env.pragmas,preflight,auth:'Private loopback fixture token; fixed trusted owner, not production authentication.'}));
