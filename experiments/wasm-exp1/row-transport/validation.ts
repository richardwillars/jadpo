import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
// The app imports persistence: keep initialization in a private fixture database.
delete Bun.env.DATABASE_URL;Bun.env.SQLITE_PATH='build/wasm-exp1/row-transport/validation/validation.sqlite';
const app=await import('../../../build/wasm-exp1/baseline/generated/bun/target/app.ts');
const spec=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8'));
const module=new WebAssembly.Module(readFileSync('build/wasm-exp1/row-transport/validation/diagnostic.wasm'));
const core:any=new WebAssembly.Instance(module).exports,enc=new TextEncoder();
const cases=[
 {name:'row small',kind:0,type:'Item',value:spec.seeds.Item[0],valid:true},
 {name:'row large',kind:0,type:'Item',value:{...spec.seeds.Item[0],note:'x'.repeat(16384)},valid:true},
 {name:'row invalid UUID',kind:0,type:'Item',value:{...spec.seeds.Item[0],id:'bad'},valid:false},
 {name:'row invalid last field',kind:0,type:'Item',value:{...spec.seeds.Item[0],note:42},valid:false},
 {name:'title ASCII',kind:1,type:'ItemTitle',value:'abcdef',valid:true},
 {name:'title Unicode',kind:1,type:'ItemTitle',value:'é中😀é中😀',valid:true},
 {name:'title too long',kind:1,type:'ItemTitle',value:'😀'.repeat(64),valid:false},
 {name:'UUID record',kind:2,type:'User',value:{id:spec.seeds.Item[0].owner_id},valid:true},
];
const results:any[]=[];
for(const c of cases){
 core.reset();const bytes=enc.encode(JSON.stringify(c.value)),p=core.alloc(bytes.length);new Uint8Array(core.memory.buffer,p,bytes.length).set(bytes);assert.equal(core.validation_load(c.kind,p,bytes.length),1);
 const validator=(app.experimentValidators as any)[c.type];
 const batch=128;
 const bun=()=>{let accepted=0;for(let i=0;i<batch;i++)try{validator(c.value,'input');accepted++;}catch(e){if(!(e instanceof app.ValidationError))throw e;}return accepted;};
 const wasm=()=>core.validation_run(batch);
 for(let run=0;run<5;run++)for(const target of run%2?['wasm','bun']:['bun','wasm']){
  const f=target==='bun'?bun:wasm;assert.equal(f(),c.valid?batch:0);
  for(let i=0;i<20;i++)f();const start=performance.now();let count=0;
  while(performance.now()-start<100){assert.equal(f(),c.valid?batch:0);count+=batch;}
  results.push({case:c.name,valid:c.valid,target,run:run+1,checks:count,meanNs:(performance.now()-start)*1e6/count});
 }
}
writeFileSync('build/wasm-exp1/row-transport/validation.json',JSON.stringify({scope:'Diagnostic batch checks on values already resident in each runtime, 5 rotations, 0.1s samples. Excludes transfer/decode and amortises calls 128 checks at a time. Rust returns bool; Bun constructs validated records or throws ValidationError. This compares current validator implementations, not equivalent error-report construction or request latency.',results},null,2)+'\n');
