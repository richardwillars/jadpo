import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {rowCodec} from './codec.ts';
import {configureRows,invokeSync} from './selected-test-driver.ts';
const meta=JSON.parse(readFileSync(import.meta.dir+'/../read-path/compiler/build/row-codec.json','utf8'));
const bytes=readFileSync(import.meta.dir+'/../read-path/compiler/build/application.wasm');
const program=JSON.parse(readFileSync('experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const row=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8')).seeds.Item[0];
const entry=(name:string)=>program.declarations.find((d:any)=>d.name===name).semanticId;
const enc=new TextEncoder(),dec=new TextDecoder(),codec=rowCodec(meta);
function pending(flags:number){
 const core:any=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;expect(core.set_row_transport(flags)).toBe(1);
 const input=enc.encode(JSON.stringify(row.id)),p=core.alloc(input.length);new Uint8Array(core.memory.buffer,p,input.length).set(input);
 expect(core.start(entry('Item.read'),p,input.length)).toBe(2);
 return {format:core.result_format(),frame:new Uint8Array(core.memory.buffer,core.result_ptr(),core.result_len()).slice()};
}
test('compact request restores the exact checked policy, authority, predicate and fresh request handle',()=>{
 const legacy=pending(2),compact=pending(10);expect(compact.format).toBe(3);expect(compact.frame.length).toBe(86);
 expect(codec.pending(compact.frame)).toEqual(JSON.parse(dec.decode(legacy.frame)));
 const plan=meta.readPlans.find((p:any)=>p.id===entry('Item.read'));
 expect(plan.jsonBase).toBe(enc.encode(JSON.stringify({...codec.pending(compact.frame),requestId:0,operationId:0,args:{...plan.args,predicate:{...plan.args.predicate,value:null}}})).length);
 for(const offset of [0,3,4,35,44,47]){const b=compact.frame.slice();b[offset]^=128;expect(()=>codec.pending(b)).toThrow();}
 for(const [offset,value] of [[36,0],[36,2147483648],[40,0],[40,65],[44,0]]){const b=compact.frame.slice();new DataView(b.buffer).setUint32(offset,value,true);expect(()=>codec.pending(b)).toThrow();}
 expect(()=>codec.pending(compact.frame.subarray(0,48))).toThrow();
 expect(()=>codec.pending(new Uint8Array([...compact.frame,0]))).toThrow();
 const malformed=compact.frame.slice();malformed[49]=255;expect(()=>codec.pending(malformed)).toThrow();
 const expanded=new Uint8Array(65536);expanded.set(compact.frame.subarray(0,48));expanded.set(enc.encode(JSON.stringify('x'.repeat(65536-50))),48);
 expect(()=>codec.pending(expanded)).toThrow('pending-size');
});
test('descriptors cannot be changed across requests; predicate inputs stay request-local',()=>{
 const frame=pending(10).frame,first=codec.pending(frame),second=codec.pending(frame);
 expect(()=>{first.args.policy.operation.operation='mutated';}).toThrow();
 first.args.predicate.value='changed';expect(second.args.predicate.value).toBe(row.id);
 const original=meta.readPlans[0].args.entity;meta.readPlans[0].args.entity='wrong';expect(codec.pending(frame).args.entity).toBe('Item');meta.readPlans[0].args.entity=original;
});
test('all compact modes execute ordinary fresh reads and keep guest validation',()=>{
 for(const flags of [10,14,15]){
  const module=new WebAssembly.Module(bytes);configureRows(module,meta,flags);let reads=0;
  const dispatch=(cap:string,args:any)=>{reads++;expect(cap).toBe('storage.read');expect(args.freshness).toBe('authoritative');expect(args.predicate.value).toBe(row.id);return {kind:'success',value:{...row,note:'x'.repeat(16384)}};};
  for(let n=0;n<3;n++)expect(invokeSync(module,entry('Item.read'),row.id,dispatch)).toEqual({kind:'success',value:{...row,note:'x'.repeat(16384)}});
  expect(reads).toBe(3);
  expect(invokeSync(module,entry('Item.read'),row.id,()=>({kind:'success',value:{...row,title:'a'}})).kind).toBe('internal');
  expect(invokeSync(module,entry('Item.read'),row.id,()=>({kind:'success',value:null}))).toEqual({kind:'domain',failure:'ItemMissing'});
 }
});
