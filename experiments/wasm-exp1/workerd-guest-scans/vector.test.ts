import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {rowCodec} from './codec.ts';
import {configureRows,invokeSync} from './driver.ts';
const base=import.meta.dir+'/compiler/build/';
const meta=JSON.parse(readFileSync(base+'row-codec.json','utf8'));
const program=JSON.parse(readFileSync('experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const row=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8')).seeds.Item[0];
const entry=program.declarations.find((d:any)=>d.name==='Item.read').semanticId;
const codec=rowCodec(meta,true),schema=codec.schema('Item')!;
const enc=new TextEncoder(),dec=new TextDecoder();
const modules=[new WebAssembly.Module(readFileSync(import.meta.dir+'/../read-path/compiler/build/application.wasm')),...['scalar','vector','bulk'].map(n=>new WebAssembly.Module(readFileSync(base+n+'.wasm')))];
function raw(module:WebAssembly.Module){
 const c:any=new WebAssembly.Instance(module).exports;
 const write=(b:Uint8Array)=>{const p=c.alloc(b.length);new Uint8Array(c.memory.buffer,p,b.length).set(b);return [p,b.length] as const;};
 return (bytes:Uint8Array)=>{if(c.reset()!==1)throw Error('reset');c.set_row_transport(7);
  if(c.start(entry,...write(enc.encode(JSON.stringify(row.id))))!==2)throw Error('start');
  const pending=JSON.parse(dec.decode(new Uint8Array(c.memory.buffer,c.result_ptr(),c.result_len())));
  return c.resume_row_ref(pending.requestId,pending.operationId,...write(bytes));
 };
}
const runs=modules.map(raw);
function noteOffset(bytes:Uint8Array){let p=40;const v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
 for(const key of meta.records.find((r:any)=>r.id===schema).fields){const tag=bytes[p++];if(tag===2){const n=v.getUint32(p,true);p+=4;if(key==='note')return p;p+=n;}else if(tag===5)p+=8;}
 throw Error('missing text');
}

test('scalar and SIMD guests independently reject logical JSON overflow in short binary frames',()=>{
 for(const atom of ['x','é','😀','"','\\','\n','\u0000','\u001f']){
  const value={...row,note:atom.repeat(2000)};
  value.note+='x'.repeat(65536-enc.encode(JSON.stringify({kind:'success',value})).length);
  for(const extra of ['', 'x']){
   const b=codec.encode(schema,{...value,note:value.note+extra}).slice();
   for(const run of runs)expect(run(b)).toBe(extra?3:0);
  }
 }
 const b=codec.encode(schema,{...row,note:'\u0000'.repeat(11000)}).slice();
 expect(b.length).toBeLessThan(65536);for(const run of runs)expect(run(b)).toBe(3);
});

test('SIMD accepts mixed Unicode/control data and matches the old guest budget',()=>{
 let state=1729;const next=()=>state=(Math.imul(state,1664525)+1013904223)>>>0;
 for(let i=0;i<350;i++){
  const s=Array.from({length:next()%2000},()=>{let cp=next()%0x110000;if(cp>=0xd800&&cp<=0xdfff)cp=0;return String.fromCodePoint(cp);}).join('');
  const text=i%2?s:String.fromCharCode(...Array.from({length:128},(_,j)=>j)).repeat(16)+s;
  const bytes=codec.encode(schema,{...row,note:text}).slice();for(const run of runs)expect(run(bytes)).toBe(0);
 }
});

test('SIMD rejects invalid UTF-8 around vector/block boundaries and truncated tails',()=>{
 const bad=[[0x80],[0xbf],[0xc0,0xaf],[0xc1,0xbf],[0xc2,0x20],[0xe0,0x80,0xaf],[0xed,0xa0,0x80],[0xf0,0x80,0x80,0x80],[0xf4,0x90,0x80,0x80],[0xf5,0x80,0x80,0x80],[0xff],[0xe2,0x82],[0xf0,0x9f,0x98]];
 for(const offset of [0,15,16,31,32,63,64,127,508,511])for(const pattern of bad){
  const b=codec.encode(schema,{...row,note:'x'.repeat(512)}).slice(),pos=noteOffset(b);
  b.set(pattern.slice(0,512-offset),pos+offset);
  for(const run of runs)expect(run(b)).toBe(3);
 }
});

test('new host fails closed without the guest budget contract and preserves semantic failures',()=>{
 const old=modules[0];configureRows(old,meta,15);
 expect(invokeSync(old,entry,row.id,()=>({kind:'success',value:row})).kind).toBe('internal');
 for(const module of modules.slice(1)){
  configureRows(module,meta,15);
  for(const value of [{...row,note:'\u0000'.repeat(11000)},{...row,note:'\ud800'.repeat(1100)},{...row,title:'x',note:'é'.repeat(5000)}])
   expect(invokeSync(module,entry,row.id,()=>({kind:'success',value})).kind).toBe('internal');
  expect(invokeSync(module,entry,row.id,()=>({kind:'success',value:row}))).toEqual({kind:'success',value:row});
 }
});
