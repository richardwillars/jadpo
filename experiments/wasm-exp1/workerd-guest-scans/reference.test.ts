import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {rowCodec} from './codec.ts';
import {configureRows,invokeSync,invoke} from './driver.ts';
const meta=JSON.parse(readFileSync(import.meta.dir+'/./compiler/build/row-codec.json','utf8'));
const moduleBytes=readFileSync(import.meta.dir+'/./compiler/build/bulk.wasm');
const program=JSON.parse(readFileSync('experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const row=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8')).seeds.Item[0];
const entry=(name:string)=>program.declarations.find((d:any)=>d.name===name).semanticId;
const codec=rowCodec(meta),schema=codec.schema('Item')!;
const enc=new TextEncoder(),dec=new TextDecoder();
function moduleFor(flags:number){const m=new WebAssembly.Module(moduleBytes);configureRows(m,meta,flags);return m;}
function raw(flags=7){const core:any=new WebAssembly.Instance(new WebAssembly.Module(moduleBytes)).exports;core.set_row_transport(flags);
 const write=(b:Uint8Array)=>{const p=core.alloc(b.length);expect(p).toBeGreaterThan(0);new Uint8Array(core.memory.buffer,p,b.length).set(b);return [p,b.length] as const;};
 expect(core.start(entry('Item.read'),...write(enc.encode(JSON.stringify(row.id))))).toBe(2);
 const pending=JSON.parse(dec.decode(new Uint8Array(core.memory.buffer,core.result_ptr(),core.result_len())));
 return {core,write,pending,bytes:()=>new Uint8Array(core.memory.buffer,core.result_ptr(),core.result_len()).slice()};
}
test('reference requires validated unchanged row and matches schema/request/operation',()=>{
 const value={...row,note:'x'.repeat(16384)},owned=codec.snapshot(schema,value);
 const r=raw();expect(r.core.resume_row_ref(r.pending.requestId,r.pending.operationId,...r.write(codec.encode(schema,owned)))).toBe(0);
 expect(r.core.result_format()).toBe(2);expect(r.bytes().length).toBe(48);
 const key=r.pending.requestId+':'+r.pending.operationId,rows=new Map([[key,{schema,row:owned}]]);
 const output=codec.reference(schema,r.bytes(),rows);expect(output).toEqual(value);
 output.note='changed';expect(owned.note).toBe(value.note);
 for(const offset of [0,3,4,35,36,40,44]){const bad=r.bytes();bad[offset]^=1;expect(()=>codec.reference(schema,bad,rows)).toThrow();}
 expect(()=>codec.reference(schema,r.bytes(),new Map())).toThrow();
 expect(()=>codec.reference(schema+1,r.bytes(),rows)).toThrow();
 expect(()=>codec.reference(schema,r.bytes().subarray(0,47),rows)).toThrow();
 expect(()=>codec.reference(schema,new Uint8Array([...r.bytes(),0]),rows)).toThrow();
 for(const bad of [{...value,title:'x'},{...value,id:'not-a-uuid'}]){
  const t=raw();expect(t.core.resume_row(t.pending.requestId,t.pending.operationId,...t.write(codec.encode(schema,bad)))).toBe(3);
  expect(t.core.result_format()).toBe(0);
 }
});
test('snapshot prevents aliasing; transformed field and absent row use ordinary semantics',()=>{
 for(const flags of [6,7,14,15]){
  const m=moduleFor(flags),value={...row,note:'😀'.repeat(4096)};
  const output=invokeSync(m,entry('Item.read'),row.id,()=>({kind:'success',value}));
  value.note='mutated host';expect((output as any).value.note).toBe('😀'.repeat(4096));
  expect(invokeSync(m,entry('Item.read_title'),row.id,()=>({kind:'success',value}))).toEqual({kind:'success',value:row.title});
  expect(invokeSync(m,entry('Item.read'),row.id,()=>({kind:'success',value:null}))).toEqual({kind:'domain',failure:'ItemMissing'});
  expect(invokeSync(m,entry('Item.read'),row.id,()=>({kind:'success',value:{...row,note:42}})).kind).toBe('internal');
 }
});
test('concurrent and reused calls never resolve another request snapshot',async()=>{
 const m=moduleFor(7);
 for(let repeat=0;repeat<20;repeat++){
  const outputs=await Promise.all(Array.from({length:8},(_,lane)=>{
   const value={...row,note:`${repeat}-${lane}:`+'x'.repeat(16384)};
   return invoke(m,entry('Item.read'),row.id,async()=>{await Bun.sleep(8-lane);return {kind:'success',value};});
  }));
  outputs.forEach((o:any,lane)=>expect(o.value.note).toBe(`${repeat}-${lane}:`+'x'.repeat(16384)));
 }
});
test('host JSON budgets agree with canonical JSON at every ASCII control/escape boundary',()=>{
 const m=moduleFor(7);
 for(let code=0;code<128;code++){
  const prefix=String.fromCharCode(code).repeat(3),value={...row,note:prefix};
  const remaining=65536-enc.encode(JSON.stringify({kind:'success',value})).length;value.note+='x'.repeat(remaining);
  expect(enc.encode(JSON.stringify({kind:'success',value})).length).toBe(65536);
  expect(()=>codec.encode(schema,value)).not.toThrow();
  expect(invokeSync(m,entry('Item.read'),row.id,()=>({kind:'success',value}))).toEqual({kind:'success',value});
  expect(()=>codec.encode(schema,{...value,note:value.note+'x'})).toThrow();
 }
});
test('reference negotiation cannot bypass the guest buffer bound or stale-handle rejection',()=>{
 const r=raw(6),value={...row,note:'x'.repeat(4096)};
 const args=r.write(enc.encode(JSON.stringify({kind:'success',value})));
 expect(r.core.resume_ref(r.pending.requestId,r.pending.operationId,args[0],65537)).toBe(3);
 const stale=raw(6);expect(stale.core.resume_ref(stale.pending.requestId+1,stale.pending.operationId,...stale.write(enc.encode(JSON.stringify({kind:'success',value}))))).toBe(3);
 const m=moduleFor(7);expect(invokeSync(m,entry('Item.read'),row.id,()=>({kind:'success',value:{...value,note:'\n'.repeat(40000)}})).kind).toBe('internal');
});
test('checked source renaming and refinement mutation govern the reference path',()=>{
 const renamed=JSON.parse(readFileSync('build/wasm-exp1/workerd-guest-scans/mutation/renamed/projected/program.json','utf8'));
 const renamedModule=new WebAssembly.Module(readFileSync(import.meta.dir+'/./compiler/build/renamed.wasm'));
 configureRows(renamedModule,JSON.parse(readFileSync(import.meta.dir+'/./compiler/build/renamed-row-codec.json','utf8')),15);
 const {owner_id,...rest}=row,value={...rest,account_key:owner_id,note:'x'.repeat(16384)};
 expect(invokeSync(renamedModule,renamed.declarations.find((d:any)=>d.name==='Record.read').semanticId,row.id,(_cap,args:any)=>{
  expect(args.entity).toBe('Record');return {kind:'success',value};
 })).toEqual({kind:'success',value});
 const mutated=JSON.parse(readFileSync('build/wasm-exp1/workerd-guest-scans/mutation/mutated/projected/program.json','utf8'));
 const changed=new WebAssembly.Module(readFileSync(import.meta.dir+'/./compiler/build/mutated.wasm'));
 configureRows(changed,JSON.parse(readFileSync(import.meta.dir+'/./compiler/build/mutated-row-codec.json','utf8')),15);
 const host=()=>({kind:'success',value:{...row,title:'abc',note:'x'.repeat(16384)}});
 expect(invokeSync(moduleFor(7),entry('Item.read'),row.id,host).kind).toBe('success');
 expect(invokeSync(changed,mutated.declarations.find((d:any)=>d.name==='Item.read').semanticId,row.id,host).kind).toBe('internal');
});

test('ordinary binary resume never claims a host snapshot',()=>{
 const r=raw(7),value={...row,note:'x'.repeat(16384)};
 expect(r.core.resume_row(r.pending.requestId,r.pending.operationId,...r.write(codec.encode(schema,value)))).toBe(0);
 expect(r.core.result_format()).toBe(1);
 expect(codec.decode(schema,r.bytes())).toEqual(value);
});
test('snapshot reads accessors into private scalars and normalises negative zero',()=>{
 let reads=0;const value={...row,get note(){reads++;return 'x'.repeat(16384);}};
 const snapshot=codec.snapshot(schema,value);expect(reads).toBe(1);expect(snapshot.note).toBe('x'.repeat(16384));
 const shape=meta.records.find((r:any)=>r.name==='Item');
 expect(Object.is(codec.snapshot(schema,{[shape.fields[0]]:-0})[shape.fields[0]],0)).toBe(true);
 for(const flags of [6,7,14,15])expect(invokeSync(moduleFor(flags),entry('Item.read'),row.id,()=>({kind:'success',value}))).toEqual({kind:'success',value:{...row,note:'x'.repeat(16384)}});
});
