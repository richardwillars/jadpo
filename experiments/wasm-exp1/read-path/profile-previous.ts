import {rowCodec} from '../typed-values/codec.ts';
import {Database} from 'bun:sqlite';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import assert from 'node:assert/strict';
import {createStorage} from '../boundary-http/adapter.ts';
import {bunSqlite} from '../optimization/cached-sqlite.ts';
const program=JSON.parse(readFileSync('experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const spec=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8'));
const module=new WebAssembly.Module(readFileSync('experiments/wasm-exp1/typed-values/compiler/build/application.wasm'));
const db=new Database(':memory:'),storage=createStorage(program,bunSqlite(db));storage.setup();
const principal={entity:'User',values:{id:spec.seeds.principals.owner.id}};
const enc=new TextEncoder(),dec=new TextDecoder('utf-8',{fatal:true});
const codec=rowCodec(JSON.parse(readFileSync('experiments/wasm-exp1/typed-values/compiler/build/row-codec.json','utf8')));
const results:any[]=[];
for(const size of [256,4096,16384,49152])for(const escaped of [false,true]){
 const rows=structuredClone(spec.seeds.Item);rows[0].note=escaped?'é中😀"\\\n'.repeat(Math.floor(size/32)):'x'.repeat(size);
 storage.resetFixture({Item:rows});
 for(const name of ['Item.read_title','Item.read']){
  const core:any=new WebAssembly.Instance(module).exports;
  const op=program.declarations.find((d:any)=>d.name===name).semanticId;
  const totals:Record<string,number>={},counts:Record<string,number>={};let record=false;
  const timed=(key:string,fn:()=>any)=>{const t=performance.now(),v=fn();if(record)totals[key]=(totals[key]??0)+performance.now()-t;return v;};
  const write=(key:string,value:any)=>{
   const json=timed(key+'_stringify',()=>JSON.stringify(value));
   const bytes=timed(key+'_utf8',()=>enc.encode(json));counts[key+'_bytes']=bytes.length;
   const p=timed(key+'_alloc_copy',()=>{const p=core.alloc(bytes.length);assert(p);new Uint8Array(core.memory.buffer,p,bytes.length).set(bytes);return p;});return [p,bytes.length];
  };
  const read=(key:string)=>{
   const bytes=timed(key+'_copy',()=>new Uint8Array(core.memory.buffer,core.result_ptr(),core.result_len()).slice());counts[key+'_bytes']=bytes.length;
   if(core.result_format()===1)return timed('binary_host_decode',()=>({kind:'success',value:codec.decode(codec.schema('Item'),bytes)}));
   const json=timed(key+'_utf8',()=>dec.decode(bytes));return timed(key+'_parse',()=>JSON.parse(json));
  };
  const run=()=>{
   assert.equal(timed('reset',()=>core.reset()),1);core.set_row_transport(2);
   const [p,n]=write('input',rows[0].id);assert.equal(timed('guest_start',()=>core.start(op,p,n)),2);
   const pending=read('pending');const result=timed('host_sql_and_validation',()=>storage.read(pending.args,principal));
   const [rp,rn]=write('row',result);assert.equal(timed('guest_resume_decode_validate_execute_encode',()=>core.resume(pending.requestId,pending.operationId,rp,rn)),0);
   const output=read('terminal');assert.deepEqual(output,{kind:'success',value:name==='Item.read'?rows[0]:rows[0].title});
   timed('http_stringify',()=>JSON.stringify(output));
  };
  for(let i=0;i<1000;i++)run();record=true;for(let i=0;i<5000;i++)run();
  results.push({name,size,escaped,counts,meanUs:Object.fromEntries(Object.entries(totals).map(([k,v])=>[k,v*1000/5000]))});
 }
}
mkdirSync('build/wasm-exp1/read-path',{recursive:true});
const report={scope:'Instrumented frozen immutable-row module; actual full-row or title-only result. Diagnostic means, not additive production p95. Each response checked.',results};
writeFileSync('build/wasm-exp1/read-path/profile.json',JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report));db.close();
