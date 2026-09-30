import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {invoke,invokeSync,poolStats} from '../optimization/driver.ts';
import {Database} from 'bun:sqlite';
import {createStorage} from '../storage/adapter.ts';
import {bunSqlite} from '../storage/bun-sqlite.ts';
const program=JSON.parse(readFileSync('experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
const spec=JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8'));
const module=new WebAssembly.Module(readFileSync('experiments/wasm-exp1/value-path/compiler/build/application.wasm'));
const entry=program.declarations.find((d:any)=>d.name==='probe').semanticId;
const row=spec.seeds.Item[0],input={id:row.id,title:'fallback'};
const success={kind:'success',value:row};
const enc=new TextEncoder(),dec=new TextDecoder();
function raw(){const c:any=new WebAssembly.Instance(module).exports;return {c,write:(v:any)=>{const b=enc.encode(JSON.stringify(v)),p=c.alloc(b.length);new Uint8Array(c.memory.buffer,p,b.length).set(b);return [p,b.length] as const;},read:()=>JSON.parse(dec.decode(new Uint8Array(c.memory.buffer,c.result_ptr(),c.result_len())))};}
test('reset refuses pending, drops completed request and rejects a previous lease handle',()=>{
 const {c,write,read}=raw();expect(c.start(entry,...write(input))).toBe(2);const first=read();expect(c.reset()).toBe(0);
 expect(c.resume(first.requestId,first.operationId,...write(success))).toBe(0);expect(read()).toEqual({kind:'success',value:'alpha'});
 expect(c.reset()).toBe(1);expect(c.result_len()).toBe(0);expect(c.start(entry,...write(input))).toBe(2);const second=read();expect(second.requestId).toBeGreaterThan(first.requestId);
 expect(c.resume(first.requestId,first.operationId,...write(success))).toBe(3);expect(read().kind).toBe('internal');
 expect(c.reset()).toBe(1);expect(c.start(entry,...write(input))).toBe(2);const third=read();expect(c.resume(third.requestId,third.operationId,...write(success))).toBe(0);
});
test('ten thousand reused requests validate fresh values and retain bounded memory',()=>{
 const before={...poolStats};
 for(let i=0;i<10000;i++){const title=i%2?'bravo':'alpha';const r=invokeSync(module,entry,{...input,note:i%1000===0?'x'.repeat(64000):null},()=>({kind:'success',value:{...row,title}}));expect(r).toEqual({kind:'success',value:title});}
 expect(poolStats.created-before.created).toBeLessThanOrEqual(1);expect(poolStats.reused-before.reused).toBeGreaterThanOrEqual(9999);expect(poolStats.active).toBe(0);expect(poolStats.maxRetainedPages).toBeGreaterThan(0);expect(poolStats.maxRetainedPages).toBeLessThanOrEqual(32);expect(poolStats.maxIdle).toBeLessThanOrEqual(8);
});
test('eight simultaneous suspended leases never exchange principals or rows',async()=>{
 const db=new Database(':memory:');const storage=createStorage(program,bunSqlite(db));storage.setup();storage.resetFixture({Item:spec.seeds.Item});
 for(let n=0;n<25;n++){
  let reached=0;let release!:()=>void;const barrier=new Promise<void>(r=>release=r);
  const values=await Promise.all(Array.from({length:8},(_,i)=>invoke(module,entry,input,async(_cap,args)=>{if(++reached===8)release();await barrier;await Bun.sleep(7-i);const principal=spec.seeds.principals[i%2?'other':'owner'];return storage.read(args,{entity:principal.entity,values:{id:principal.id}});})));values.forEach((v,i)=>expect(v).toEqual({kind:'success',value:i%2?'fallback':'alpha'}));
 }
 expect(poolStats.peakActive).toBeGreaterThanOrEqual(8);expect(poolStats.active).toBe(0);db.close();
});
test('invalid, domain, malformed host and size faults cannot poison a following lease',()=>{
 const change=program.declarations.find((d:any)=>d.name==='Item.change').semanticId;
 expect(invokeSync(module,entry,{...input,id:'bad'},()=>{throw Error('no call');}).kind).toBe('invalid');
 expect(invokeSync(module,change,[row.id,{}],()=>({kind:'success',value:{status:'empty'}})).kind).toBe('domain');
 for(const dispatch of [()=>({kind:'nonsense'}),()=>({kind:'success',value:{...row,note:42}}),()=>({kind:'success',value:{...row,note:'x'.repeat(65536)}}),()=>{throw Error('PRIVATE_SENTINEL');}]){
  const before=poolStats.discarded;const result=invokeSync(module,entry,input,dispatch as any);expect(result.kind).toBe('internal');expect(JSON.stringify(result)).not.toContain('PRIVATE_SENTINEL');expect(poolStats.discarded).toBeGreaterThan(before);
  expect(invokeSync(module,entry,input,()=>success)).toEqual({kind:'success',value:'alpha'});
 }
 expect(poolStats.active).toBe(0);
});
