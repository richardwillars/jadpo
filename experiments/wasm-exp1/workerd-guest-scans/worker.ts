// Local-only host comparison. This is test orchestration, not a production API.
import {profileStages} from './profile.ts';
import {DurableObject} from 'cloudflare:workers';
import candidate from './candidate.wasm';
import previous from './previous.wasm';
import typed from './typed.wasm';
import views from './views.wasm';
import fused from './fused.wasm';
import adaptive from './adaptive.wasm';
import bounded from './bounded.wasm';
import direct from './direct.wasm';
import hybrid from './hybrid.wasm';
import scan from './scan.wasm';
import scalar from './scalar.wasm';
import bulk from './bulk.wasm';
import bulkJson from './bulkJson.wasm';
import vector from './vector.wasm';
import scalarSample from './scalarSample.wasm';
import vectorSample from './vectorSample.wasm';
import {configureRows as configureGuest,invokeSync as invokeGuest,poolStats as guestPoolStats} from './driver.ts';
import {configureRows as configureHybrid,invokeSync as invokeHybrid,poolStats as hybridPoolStats} from '../workerd-native-values/hybrid-driver.ts';
import {configureRows as configureNew,invokeSync as invokeNew,poolStats as newPoolStats} from '../workerd-boundary/driver.ts';
import {configureRows as configureNext,invokeSync as invokeNext,poolStats as nextPoolStats} from '../workerd-native-values/driver.ts';
import {configureRows,invokeSync,poolStats} from '../read-path/driver.ts';
import {configureRows as configurePrevious,invokeSync as invokePrevious} from '../typed-values/driver.ts';
import metadata from '../read-path/compiler/build/row-codec.json';
import previousMetadata from '../typed-values/compiler/build/row-codec.json';
import program from '../compiler/build/projected/program.json';
import spec from '../acceptance.json';
import {createStorage} from '../boundary-http/adapter.ts';
import {cloudflareSqlite} from '../storage/cloudflare-sqlite.ts';
import {experimentCallables,experimentValidators,captureOperation,DomainFailure,ValidationError} from '../../../build/wasm-exp1/baseline/generated/bun/target/app.ts';
const allowedTargets=['js','candidate','typed','views','fused','adaptive','bounded','direct','hybrid','scan','scalar','vector','scalarSample','vectorSample','bulk','bulkJson','noop'] as const;
type Target=typeof allowedTargets[number];
type ReadOperation='Item.read'|'Item.read_title'|'probe';
type Call={target:Target;operation:ReadOperation;input:unknown;principal?:'owner'|'other'};
const declarations=new Map(program.declarations.map(d=>[d.name,d]));
configurePrevious(previous,previousMetadata,2);
// Use separate Module identities so different negotiated modes cannot collide.
// Cloudflare's imported module is precompiled; bytes are never compiled at runtime.
configureRows(candidate,metadata,14);
configureRows(typed,metadata,15);
configureNew(views,metadata,14);configureNew(fused,metadata,15);configureNext(adaptive,metadata,15,true,true);configureNext(bounded,metadata,15);configureNext(direct,metadata,15,false,true);configureHybrid(hybrid,metadata,15,true,true);configureHybrid(scan,metadata,15,true,false);
if(new Set([candidate,typed,views,fused,adaptive,bounded,direct,hybrid,scan]).size!==9)throw Error('Separate mode module identities required');
configureGuest(bulk,metadata,15);configureGuest(bulkJson,metadata,14);configureGuest(scalar,metadata,15);configureGuest(vector,metadata,15);configureGuest(scalarSample,metadata,15,true,true);configureGuest(vectorSample,metadata,15,true,true);
if(new Set([scalar,vector,scalarSample,vectorSample]).size!==4)throw Error('Separate guest module identities');
function freeze<T>(v:T):T {if(v&&typeof v==='object'){Object.values(v).forEach(freeze);Object.freeze(v);}return v;}
const plans=new Map(metadata.readPlans.map(p=>[p.args.operation,freeze(structuredClone(p.args))]));
export class ReadAuthority extends DurableObject<Env> {
 private storage:ReturnType<typeof createStorage>;
 private count=0;
 private note:string|null='';
 constructor(ctx:DurableObjectState,env:Env){
  super(ctx,env);this.storage=createStorage(program,cloudflareSqlite(ctx.storage));this.storage.setup();
 }
 reset(note:string|null){
  if(note!==null&&(typeof note!=='string'||new TextEncoder().encode(JSON.stringify(note)).length>64000))throw Error('fixture bound');
  const rows=structuredClone(spec.seeds.Item);rows[0].note=note;
  this.storage.resetFixture({Item:rows});this.note=note;this.count=0;
  return {snapshot:this.storage.snapshot(),runtime:{hasBun:'Bun' in globalThis,hasNodeProcess:'process' in globalThis},sqlite:'Platform-managed SQLite; sqlite_version() is not exposed by this host'};
 }
 begin(){this.count=0;return {ok:true};}
 stats(){return {count:this.count,snapshot:this.storage.snapshot(),poolStats:{...poolStats},newPoolStats:{...newPoolStats},nextPoolStats:{...nextPoolStats},hybridPoolStats:{...hybridPoolStats},guestPoolStats:{...guestPoolStats}};}
 changeOwner(){this.ctx.storage.sql.exec('UPDATE entity_Item SET owner_id=? WHERE id=?',spec.seeds.principals.other.id,spec.seeds.Item[0].id);return {ok:true};}
 async batch(call:Call,iterations:number){
  if(!Number.isInteger(iterations)||iterations<1||iterations>2000)throw Error('batch bound');
  let last:any;const row={...spec.seeds.Item[0],note:this.note};
  const start=performance.now();
  for(let i=0;i<iterations;i++){last=await this.call(call);if(last.kind!=='success'||(call.operation==='Item.read'?Object.keys(last.value).length!==Object.keys(row).length||Object.entries(row).some(([k,v])=>last.value[k]!==v):last.value!==row.title))throw Error('batch response');}
  return {last,iterations,elapsedMs:performance.now()-start};
 }
 profile(options:{flags:number;size:number;kind:string;operation:string}){
  return profileStages(this.storage, candidate, options);
 }
 async call(call:Call){
  const {target,operation,input,principal:principalName='owner'}=call;
  if(!allowedTargets.includes(target)||!['Item.read','Item.read_title','probe'].includes(operation)||!['owner','other'].includes(principalName))throw Error('fixture operation');
  this.count++;
  const principal=spec.seeds.principals[principalName],trusted={entity:principal.entity,values:{id:principal.id}};
  if(target==='noop')return {kind:'success',value:operation==='Item.read'?{...spec.seeds.Item[0],note:this.note}:spec.seeds.Item[0].title};
  if(target==='js'){
   let argument:unknown;
   try{argument=operation==='probe'?experimentValidators.ProbeInput(input,'input'):experimentValidators.User({id:input},'input').id;}
   catch(error){if(error instanceof ValidationError)return {kind:'invalid'};throw error;}
   const storage=this.storage;
   // Same authority, scoped query and host-row validation as the WASM variants.
   const port=(name:string)=>({
    withPolicy:(_principal:unknown,next:string)=>port(next),
    query_required_Item_by_id:(id:string)=>{
     const plan=plans.get(name);if(!plan)throw Error('checked read plan');
     return storage.read({...plan,predicate:{...plan.predicate,value:id}},trusted).value;
    },
   });
   try{return {kind:'success',value:await experimentCallables[operation](argument,captureOperation(null,{kind:'user',subject:principal.id,values:{user_id:principal.id}}),port(operation))};}
   catch(error){if(error instanceof DomainFailure)return {kind:'domain',failure:error.failureName};return {kind:'internal'};}
  }
  const execute=['scalar','vector','scalarSample','vectorSample','bulk','bulkJson'].includes(target)?invokeGuest:['hybrid','scan'].includes(target)?invokeHybrid:['bounded','direct','adaptive'].includes(target)?invokeNext:['views','fused'].includes(target)?invokeNew:invokeSync;
  return execute(target==='bulk'?bulk:target==='bulkJson'?bulkJson:target==='scalar'?scalar:target==='vector'?vector:target==='scalarSample'?scalarSample:target==='vectorSample'?vectorSample:target==='hybrid'?hybrid:target==='scan'?scan:target==='bounded'?bounded:target==='direct'?direct:target==='adaptive'?adaptive:target==='views'?views:target==='fused'?fused:target==='typed'?typed:candidate,declarations.get(operation)!.semanticId,input,(cap,args)=>{
   if(cap!=='storage.read')throw Error('read-only experiment');return this.storage.read(args,trusted);
  });
 }
}
export default {
 async fetch(request:Request,env:Env){
  if(request.method!=='POST'||request.headers.get('x-experiment-token')!==env.EXPERIMENT_TOKEN)return new Response('Not found',{status:404});
  const reader=request.body?.getReader();if(!reader)return new Response('Missing body',{status:400});
  const chunks:Uint8Array[]=[];let size=0;
  for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>65536){await reader.cancel();return new Response('Too large',{status:413});}chunks.push(value);}
  const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
  const input=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
  const {action,key}=input;if(typeof key!=='string'||!/^run-[a-z0-9-]{1,80}$/.test(key))return new Response('Invalid fixture',{status:400});
  const authority=env.READ_AUTHORITY.getByName(key);
  let output;
  if(action==='reset')output=await authority.reset(input.note);
  else if(action==='begin')output=await authority.begin();
  else if(action==='stats')output=await authority.stats();
  else if(action==='changeOwner')output=await authority.changeOwner();
  else if(action==='batch')output=await authority.batch(input.call,input.iterations);
  else if(action==='profile')output=await authority.profile(input.options);
  else if(action==='call')output=await authority.call(input.call);
  else return new Response('Not found',{status:404});
  return Response.json(output);
 }
} satisfies ExportedHandler<Env>;
