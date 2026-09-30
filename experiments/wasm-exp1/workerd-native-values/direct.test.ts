import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {rowCodec} from './codec.ts';
import {rowCodec as reference} from '../read-path/codec.ts';
import {configureRows as configureHybrid,invokeSync as invokeHybrid} from './hybrid-driver.ts';
import {configureRows,invoke,invokeSync,poolStats} from './driver.ts';
import {configureRows as configureOld,invokeSync as invokeOld} from '../workerd-boundary/driver.ts';
const meta={version:1,digest:'a'.repeat(64),records:[{id:1,name:'Record',fields:['text','other','integer','flag']}],entries:{'1':1}};
const codec=rowCodec(meta),old=reference(meta),encoder=new TextEncoder();

test('direct encoder agrees at exact JSON bounds, with offset views and guard bytes',()=>{
 for(const atom of ['x','é','中','😀','"','\\','\n','\u0000','\u001f','\ufeff']){
  const row={text:atom.repeat(1700),other:'"\\\n'.repeat(30),integer:-9007199254740991,flag:false};
  row.text+='x'.repeat(65536-encoder.encode(JSON.stringify({kind:'success',value:row})).length);
  for(const delta of [-1,0,1]){
   const value={...row,text:delta<0?row.text.slice(0,-1):row.text+'x'.repeat(delta)};
   const backing=new Uint8Array(65536+16).fill(173),view=backing.subarray(8,-8);
   let expected:Uint8Array|undefined,actual:Uint8Array|undefined;
   try{expected=old.encode(1,value).slice();}catch{}
   try{actual=codec.encode(1,value,view).slice();}catch{}
   expect(actual).toEqual(expected);expect([...backing.subarray(0,8)]).toEqual(Array(8).fill(173));expect([...backing.subarray(-8)]).toEqual(Array(8).fill(173));
  }
 }
 expect(()=>codec.encode(1,{text:'😀'.repeat(1000)},new Uint8Array(100))).toThrow();
 expect(()=>codec.encode(1,{text:'\ud800'},new Uint8Array(65536))).toThrow();
});

test('ambiguous budget reads each accessor once and excludes inherited fields',()=>{
 let reads=0;
 const row=Object.create({other:'\u0000'.repeat(30000)});
 Object.defineProperty(row,'text',{enumerable:true,get(){reads++;return 'x'.repeat(20000);}});
 expect(codec.decode(1,codec.encode(1,row))).toEqual({text:'x'.repeat(20000)});expect(reads).toBe(1);
});

const root='experiments/wasm-exp1/',bytes=readFileSync(root+'read-path/compiler/build/application.wasm');
const metadata=JSON.parse(readFileSync(root+'read-path/compiler/build/row-codec.json','utf8'));
const program=JSON.parse(readFileSync(root+'compiler/build/projected/program.json','utf8'));
const row=JSON.parse(readFileSync(root+'acceptance.json','utf8')).seeds.Item[0];
const entry=program.declarations.find((d:any)=>d.name==='Item.read').semanticId;
const titleEntry=program.declarations.find((d:any)=>d.name==='Item.read_title').semanticId;
const baseline=new WebAssembly.Module(bytes);configureOld(baseline,metadata,14);

test('direct and adaptive writes preserve validation, exact fallback and repeated memory growth',()=>{
 for(const adaptive of [false,true]){
  const module=new WebAssembly.Module(bytes);configureRows(module,metadata,15,adaptive,true);
  for(let round=0;round<4;round++)for(const note of [null,'x'.repeat(1023),'x'.repeat(1024),'x'.repeat(49152),'é中😀'.repeat(4000),'"\\\n'.repeat(3000),'\u0000'.repeat(11000),'é'.repeat(33000),'\ud800'.repeat(1024),42]){
   for(const operation of [entry,titleEntry]){
    const dispatch=()=>({kind:'success',value:{...row,note}});
    expect(invokeSync(module,operation,row.id,dispatch)).toEqual(invokeOld(baseline,operation,row.id,dispatch));
   }
  }
  const exact={...row,note:''};exact.note='x'.repeat(65536-encoder.encode(JSON.stringify({kind:'success',value:exact})).length);
  for(const value of [exact,{...exact,note:exact.note+'x'},{...row,title:'',note:'x'.repeat(2048)},{...row,owner_id:'invalid',note:'x'.repeat(2048)}]){
   const dispatch=()=>({kind:'success',value});
   expect(invokeSync(module,entry,row.id,dispatch)).toEqual(invokeOld(baseline,entry,row.id,dispatch));
  }
 }
 expect(poolStats.active).toBe(0);
});

test('direct writes keep concurrent request snapshots private across suspended leases',async()=>{
 const module=new WebAssembly.Module(bytes);configureRows(module,metadata,15,false,true);
 for(let round=0;round<10;round++){
  let count=0,release!:()=>void;const barrier=new Promise<void>(r=>release=r);
  const results=await Promise.all(Array.from({length:8},(_,i)=>invoke(module,entry,row.id,async()=>{
   if(++count===8)release();await barrier;await Bun.sleep(7-i);
   return {kind:'success',value:{...row,note:String(i)+'é中😀"\\\n'.repeat(512)}};
  })));
  results.forEach((result,i)=>expect(result).toEqual({kind:'success',value:{...row,note:String(i)+'é中😀"\\\n'.repeat(512)}}));
 }
 expect(poolStats.active).toBe(0);expect(poolStats.maxRetainedPages).toBeLessThanOrEqual(32);
});


test('sampled selection preserves values with escapes before, at and beyond the sample',()=>{
 const module=new WebAssembly.Module(bytes);configureHybrid(module,metadata,15,true,true);
 for(const prefix of [0,255,256,257,1024,8192,49000])for(const escape of ['"','\\','\n','\u0000','😀']){
  const value={...row,note:'x'.repeat(prefix)+escape+'y'.repeat(2048)};
  const dispatch=()=>({kind:'success',value});
  for(const operation of [entry,titleEntry])expect(invokeHybrid(module,operation,row.id,dispatch)).toEqual(invokeOld(baseline,operation,row.id,dispatch));
 }
});
