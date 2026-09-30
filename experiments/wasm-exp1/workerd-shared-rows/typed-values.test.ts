import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {configureRows,invokeSync} from '../workerd-guest-scans/driver.ts';
const base=import.meta.dir+'/./compiler/build/';
const bytes=readFileSync(base+'shared.wasm'),meta=JSON.parse(readFileSync(base+'row-codec.json','utf8'));
const program=JSON.parse(readFileSync('experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const row=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8')).seeds.Item[0];
const entry=program.declarations.find((d:any)=>d.name==='Item.read').semanticId;
const json=new WebAssembly.Module(bytes),typed=new WebAssembly.Module(bytes);
configureRows(json,meta,0);configureRows(typed,meta,2);
const enc=new TextEncoder(),dec=new TextDecoder();
function rawResume(payload:string){
 const core:any=new WebAssembly.Instance(typed).exports;core.set_row_transport(2);
 const write=(s:string)=>{const b=enc.encode(s),p=core.alloc(b.length);expect(p).toBeGreaterThan(0);new Uint8Array(core.memory.buffer,p,b.length).set(b);return [p,b.length] as const;};
 expect(core.start(entry,...write(JSON.stringify(row.id)))).toBe(2);
 const pending=JSON.parse(dec.decode(new Uint8Array(core.memory.buffer,core.result_ptr(),core.result_len())));
 const status=core.resume(pending.requestId,pending.operationId,...write(payload));
 return {status,format:core.result_format(),length:core.result_len()};
}
test('immutable row path preserves reference results on JSON ingress, including escaping and exact budgets',()=>{
 for(const unit of ['x','é中😀','"\\\n\r\t\0','\ufeff'])for(const size of [0,1,256,1024,4096,16384]){
  const note=unit.repeat(Math.floor(size/unit.length));const value={...row,note};
  const dispatch=()=>({kind:'success',value});
  expect(invokeSync(typed,entry,row.id,dispatch)).toEqual(invokeSync(json,entry,row.id,dispatch));
 }
 for(const note of [null,42,{},true]){
  const dispatch=()=>({kind:'success',value:{...row,note}});
  expect(invokeSync(typed,entry,row.id,dispatch)).toEqual(invokeSync(json,entry,row.id,dispatch));
 }
 const envelope={kind:'success',value:{...row,note:''}};
 const capacity=65536-enc.encode(JSON.stringify(envelope)).length;
 envelope.value.note='x'.repeat(capacity);
 expect(rawResume(JSON.stringify(envelope))).toMatchObject({status:0,format:1});
 const dispatch=()=>({...envelope,value:{...envelope.value,note:envelope.value.note+'x'}});
 expect(invokeSync(typed,entry,row.id,dispatch).kind).toBe('internal');
});
test('size provenance comes from accepted bytes, never a claimed host bound',()=>{
 const envelope={kind:'success',value:{...row,note:'x'.repeat(4096)}};
 for(const payload of [JSON.stringify({...envelope,json_bound:1}),JSON.stringify({...envelope,value:{...envelope.value,json_bound:1}})])expect(rawResume(payload).status).toBe(3);
 // JSON spelling and order can differ; canonical output can only shrink here.
 const original=JSON.stringify(envelope);
 const noncanonical=original.replace('"kind"','"k\\u0069nd"').replace('"note"','"n\\u006fte"').replace('"success"','"succe\\u0073s"').replace('"value":','"value" : ');
 expect(rawResume(noncanonical)).toMatchObject({status:0,format:1});
 const duplicate=original.replace('"note":','"note":"discarded","note":');
 expect(rawResume(duplicate)).toMatchObject({status:0,format:1});
 expect(rawResume(original+' trailing').status).toBe(3);
});
test('a fresh request cannot reuse a prior row or its size proof',()=>{
 for(let n=0;n<200;n++){
  const value={...row,note:(n%2?'😀':'"\\').repeat(1000+n)};
  expect(invokeSync(typed,entry,row.id,()=>({kind:'success',value}))).toEqual({kind:'success',value});
  expect(invokeSync(typed,entry,row.id,()=>({kind:'success',value:{...value,note:'\n'.repeat(40000)}})).kind).toBe('internal');
  expect(invokeSync(typed,entry,row.id,()=>({kind:'success',value:null}))).toEqual({kind:'domain',failure:'ItemMissing'});
 }
});
