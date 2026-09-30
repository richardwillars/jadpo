import {test,expect} from 'bun:test';
import {rowCodec} from '../workerd-guest-scans/codec.ts';
import {rowCodec as original} from '../read-path/codec.ts';
import {readFileSync} from 'node:fs';
import {configureRows,invokeSync} from '../workerd-guest-scans/driver.ts';
import {configureRows as configureOld,invokeSync as invokeOld} from '../read-path/driver.ts';
const metadata={version:1,digest:'a'.repeat(64),records:[{id:1,name:'Record',fields:['é😀','quote"','flag','integer','empty']}],entries:{'1':1}};
const codec=rowCodec(metadata),baseline=original(metadata),encoder=new TextEncoder();

test('fused JSON budget agrees with reference for Unicode keys, scalars and exact limits',()=>{
 for(const text of ['x','é','中','😀','"','\\','\n','\u0000','\u001f','\u2028','\ufeff']){
  const row:any={'é😀':text.repeat(1000),'quote"':null,flag:false,integer:-9007199254740991};
  const remaining=65536-encoder.encode(JSON.stringify({kind:'success',value:row})).length;
  row['é😀']+='x'.repeat(remaining);
  for(const delta of [-1,0,1]){
   const value={...row,'é😀':delta<0?row['é😀'].slice(0,-1):row['é😀']+'x'.repeat(delta)};
   let before:Uint8Array|undefined,after:Uint8Array|undefined;
   try{before=baseline.encode(1,value).slice();}catch{}
   try{after=codec.encode(1,value).slice();}catch{}
   expect(after).toEqual(before); // Includes frames that exceed binary capacity.
  }
 }
});

test('fused frames match reference over deterministic mixed scalar and escaping cases',()=>{
 let seed=701;
 const next=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed;};
 for(let i=0;i<400;i++){
  const text=Array.from({length:next()%200},()=>String.fromCodePoint([next()%128,0x00e9,0x4e2d,0x1f600,0xfeff][next()%5])).join('');
  const row:any={'é😀':text,'quote"':i%2?null:text,flag:i%3===0,integer:i%2?Number.MAX_SAFE_INTEGER:-i};
  if(i%3===0)row.empty='';
  expect(codec.encode(1,row).slice()).toEqual(baseline.encode(1,row));
 }
 expect(()=>codec.encode(1,{'é😀':'\ud800'})).toThrow();
 expect(()=>codec.encode(1,{'é😀':'\udfff'})).toThrow();
});

test('adaptive ingress and direct JSON writes preserve faults, UTF-8 bounds and reuse',()=>{
 const base=import.meta.dir+'/./compiler/build/';
 const bytes=readFileSync(base+'shared.wasm'),meta=JSON.parse(readFileSync(base+'row-codec.json','utf8'));
 const program=JSON.parse(readFileSync('experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
 const row=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8')).seeds.Item[0];
 const entry=program.declarations.find((d:any)=>d.name==='Item.read').semanticId;
 const before=new WebAssembly.Module(bytes),after=new WebAssembly.Module(bytes);
 configureOld(before,meta,15);configureRows(after,meta,15,true);
 for(const note of [null,'','x'.repeat(16384),'é中😀'.repeat(4000),'"\\\n'.repeat(2000),'\n'.repeat(34000),'é'.repeat(34000),42]){
  const dispatch=()=>({kind:'success',value:{...row,note}});
  expect(invokeSync(after,entry,row.id,dispatch)).toEqual(invokeOld(before,entry,row.id,dispatch));
 }
 for(const input of ['é'.repeat(40000),'😀'.repeat(20000),'x'.repeat(65536),row.id]){
  const dispatch=()=>({kind:'success',value:row});
  expect(invokeSync(after,entry,input,dispatch)).toEqual(invokeOld(before,entry,input,dispatch));
 }
});
