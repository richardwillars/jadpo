import {Database} from 'bun:sqlite';
import {profile} from 'bun:jsc';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {invokeSync} from '../optimization/driver.ts';
import {createStorage} from '../optimization/cached-adapter.ts';
import {bunSqlite} from '../optimization/cached-sqlite.ts';
const root=resolve(import.meta.dir,'../../..'),out=root+'/build/wasm-exp1/boundary-http';mkdirSync(out,{recursive:true});
const program=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const spec=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/acceptance.json','utf8'));
const module=new WebAssembly.Module(readFileSync(root+'/experiments/wasm-exp1/optimization/compiler/build/application.wasm'));
const op=program.declarations.find((d:any)=>d.name==='probe').semanticId;
const db=new Database(':memory:'),storage=createStorage(program,bunSqlite(db));storage.setup();storage.resetFixture({Item:spec.seeds.Item});
const principal={entity:'User',values:{id:spec.seeds.principals.owner.id}},row=spec.seeds.Item[0];
const input={id:row.id,title:'fallback',note:'x'.repeat(175)};
let descriptor:any;invokeSync(module,op,input,(_c,a)=>{descriptor=a;return storage.read(a,principal);});
const canonical=(v:any)=>JSON.stringify(v,(_k,x)=>x!==null&&typeof x==='object'&&!Array.isArray(x)?Object.fromEntries(Object.entries(x).sort(([a],[b])=>a.localeCompare(b))):x);
const expected=canonical(descriptor.policy),enc=new TextEncoder(),dec=new TextDecoder();
const results:any[]=[];
for(const size of [256,16384]){
 input.note='x'.repeat(size-JSON.stringify({...input,note:''}).length);
 const payload=JSON.stringify(input),response=JSON.stringify({kind:'success',value:{...row,note:input.note}});
 const cases:any={policy_canonical:()=>canonical(descriptor.policy)===expected,input_json_roundtrip:()=>JSON.parse(dec.decode(enc.encode(payload))).note.length===input.note.length,row_json_roundtrip:()=>JSON.parse(dec.decode(enc.encode(response))).value.note.length===input.note.length,unconstrained_text_scalar_count:()=>[...input.note].length===input.note.length,cached_storage:()=>storage.read(descriptor,principal).kind==='success',pooled_full:()=>invokeSync(module,op,input,(_c,a)=>storage.read(a,principal)).value==='alpha'};
 for(const [name,call] of Object.entries(cases) as [string,()=>boolean][]){for(let i=0;i<2000;i++)if(!call())throw Error(name);let n=0;const start=performance.now();do{for(let i=0;i<256;i++){if(!call())throw Error(name);n++;}}while(performance.now()-start<350);results.push({size,name,n,meanUs:(performance.now()-start)*1000/n});}
}
input.note='x'.repeat(175);
const sampled=profile(()=>{for(let i=0;i<75000;i++)if(invokeSync(module,op,input,(_c,a)=>storage.read(a,principal)).value!=='alpha')throw Error('result');},100);
writeFileSync(out+'/sampling.json',JSON.stringify(sampled,null,2));
// Diagnostic direct ABI timings: all checks remain in the production driver for final tests.
const stages:any[]=[];
for(const size of [256,16384]){
 input.note='x'.repeat(size-JSON.stringify({...input,note:''}).length);
 const core:any=new WebAssembly.Instance(module).exports,totals:any={};
 const timed=(name:string,fn:()=>any)=>{const t=performance.now(),v=fn();totals[name]=(totals[name]??0)+performance.now()-t;return v;};
 const write=(v:any)=>{const b=enc.encode(JSON.stringify(v)),ptr=core.alloc(b.length);new Uint8Array(core.memory.buffer,ptr,b.length).set(b);return [ptr,b.length];};
 const read=()=>JSON.parse(dec.decode(new Uint8Array(core.memory.buffer,core.result_ptr(),core.result_len()).slice()));
 for(let i=0;i<20000;i++){
  if(timed('reset',()=>core.reset())!==1)throw Error('reset');
  const [p,n]=timed('js_encode_input_and_copy',()=>write(input));
  if(timed('guest_start_decode_validate_build_emit',()=>core.start(op,p,n))!==2)throw Error('start');
  const pending=timed('js_decode_pending',read);
  const result=timed('host_storage_validate_sql',()=>storage.read(pending.args,principal));
  const [rp,rn]=timed('js_encode_row_and_copy',()=>write(result));
  if(timed('guest_resume_decode_validate_logic_emit',()=>core.resume(pending.requestId,pending.operationId,rp,rn))!==0)throw Error('resume');
  if(timed('js_decode_terminal',read).value!=='alpha')throw Error('output');
 }
 stages.push({size,requests:20000,meanUs:Object.fromEntries(Object.entries(totals).map(([k,v])=>[k,(v as number)*1000/20000]))});
}
const report={scope:'Diagnostic attribution only; instrumentation and isolated means are not additive uninstrumented p95 estimates.',results,stages};writeFileSync(out+'/profile.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));console.log(sampled.functions);db.close();
