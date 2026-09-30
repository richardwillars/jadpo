import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {rowCodec} from './codec.ts';
import {environment,originalRows,program,spec} from './environment.ts';
import {configureRows,invokeSync,invoke,poolStats} from './driver.ts';
const meta=JSON.parse(readFileSync(import.meta.dir+'/compiler/build/row-codec.json','utf8'));
const module=new WebAssembly.Module(readFileSync(import.meta.dir+'/compiler/build/application.wasm'));
configureRows(module,meta);
const codec=rowCodec(meta),id=codec.schema('Item')!,row=originalRows[0];
const entry=program.declarations.find((d:any)=>d.name==='Item.read').semanticId;
const input={id:row.id,title:'fallback'};
const enc=new TextEncoder(),dec=new TextDecoder();
function raw(){const c:any=new WebAssembly.Instance(module).exports;c.set_row_transport(3);const write=(bytes:Uint8Array)=>{const p=c.alloc(bytes.length);expect(p).toBeGreaterThan(0);new Uint8Array(c.memory.buffer,p,bytes.length).set(bytes);return [p,bytes.length] as const;};expect(c.start(entry,...write(enc.encode(JSON.stringify(row.id))))).toBe(2);const pending=JSON.parse(dec.decode(new Uint8Array(c.memory.buffer,c.result_ptr(),c.result_len())));return {c,write,pending};}

test('round trips preserve null, BOM, Unicode, escapes and long rows across every mode',async()=>{
 // Pinned Bun 1.2.20 strips an initial BOM even in SELECT ?; exercise BOM below
 // through a direct host result, without attributing that storage bug to the codec.
 const values=[null,'','x'.repeat(16384),'x'.repeat(49152),'😀'.repeat(4096),'é中😀"\\\n\r\t\u0000'.repeat(700)];
 for(const target of ['previous','json','ingress','egress','candidate']){
  const e=await environment(target,':memory:');e.reset();
  try{for(const note of values){
   const expected={kind:'success',value:{...row,note}};
   expect(await e.call('Item.change',[row.id,{note}])).toEqual(expected);
   expect(await e.call('Item.read',row.id)).toEqual(expected);
   expect(await e.call('Item.read',row.id,'other')).toEqual({kind:'domain',failure:'ItemMissing'});
   expect(await e.call('Item.read_title',row.id)).toEqual({kind:'success',value:row.title});
  }}finally{e.close();}
 }
});
test('direct host rows preserve an initial BOM through binary ingress and egress',()=>{
 const value={...row,note:'\ufeff'+'x'.repeat(16384)};
 expect(invokeSync(module,entry,row.id,()=>({kind:'success',value}))).toEqual({kind:'success',value});
});
test('schema hash, version, row type, tags, lengths, truncation and trailing bytes fail closed',()=>{
 const good=codec.encode(id,row).slice();
 const wrongHash=good.slice();wrongHash[4]^=1;
 const wrongVersion=good.slice();wrongVersion[3]=50;
 const wrongSchema=good.slice();new DataView(wrongSchema.buffer).setUint32(36,999,true);
 const badTag=good.slice();badTag[40]=255;
 const badLength=good.slice();new DataView(badLength.buffer).setUint32(41,0xffffffff,true);
 const badUtf8=good.slice();badUtf8[45]=0xff;
 const badType=good.slice();badType[40]=3;
 const cases=[wrongHash,wrongVersion,wrongSchema,badTag,badLength,badUtf8,badType,good.slice(0,39),good.slice(0,-1),new Uint8Array([...good,0])];
 for(const bytes of cases){
  const {c,write,pending}=raw();expect(c.resume_row(pending.requestId,pending.operationId,...write(bytes))).toBe(3);
  expect(c.result_format()).toBe(0);expect(JSON.parse(dec.decode(new Uint8Array(c.memory.buffer,c.result_ptr(),c.result_len()))).kind).toBe('internal');
  expect(c.reset()).toBe(1);
 }
 for(const bytes of cases.filter(b=>b!==badType))expect(()=>codec.decode(id,bytes)).toThrow();
});
test('full JSON-size contract remains enforced at exact limit, including escaped text',()=>{
 const overhead=enc.encode(JSON.stringify({kind:'success',value:{...row,note:''}})).length;
 for(const length of [65535-overhead,65536-overhead,65537-overhead]){
  const value={...row,note:'x'.repeat(length)};
  const result=invokeSync(module,entry,row.id,()=>({kind:'success',value}));
  expect(result.kind).toBe(length+overhead<=65536?'success':'internal');
 }
 // Binary would fit, but the old escaped JSON frame would not.
 const value={...row,note:'\0'.repeat(12000)};
 expect(invokeSync(module,entry,row.id,()=>({kind:'success',value})).kind).toBe('internal');
 // Bypass host encoder: the guest must independently enforce the JSON size bound.
 const bytes=codec.encode(id,{...row,note:'x'.repeat(12000)}).slice();
 const noteStart=bytes.length-12000;bytes.fill(0,noteStart);
 const {c,write,pending}=raw();expect(c.resume_row(pending.requestId,pending.operationId,...write(bytes))).toBe(3);
});
test('invalid fields and malformed envelopes cannot be hidden by schema transport',()=>{
 for(const value of [{...row,extra:'secret'},{...row,note:42},{...row,title:'x'},{...row,id:'bad'},Object.fromEntries(Object.entries(row).filter(([k])=>k!=='title'))]){
  expect(invokeSync(module,entry,row.id,()=>({kind:'success',value})).kind).toBe('internal');
 }
 expect(invokeSync(module,entry,row.id,()=>({kind:'success',value:row,extra:1})).kind).toBe('internal');
 expect(()=>codec.encode(id,{...row,note:'\ud800'})).toThrow();
});
test('binary resume cannot replay a stale handle or resume the wrong pending operation',()=>{
 const {c,write,pending}=raw();const bytes=codec.encode(id,row).slice();
 expect(c.reset()).toBe(0);
 expect(c.resume_row(pending.requestId,pending.operationId,...write(bytes))).toBe(0);
 expect(c.reset()).toBe(1);c.set_row_transport(3);
 expect(c.start(entry,...write(enc.encode(JSON.stringify(row.id))))).toBe(2);
 expect(c.resume_row(pending.requestId,pending.operationId,...write(bytes))).toBe(3);
 expect(c.reset()).toBe(1);
});
test('interleaved async large rows retain exclusive ownership',async()=>{
 for(let iteration=0;iteration<20;iteration++){
  const results=await Promise.all(Array.from({length:8},(_,i)=>invoke(module,entry,row.id,async()=>{await Bun.sleep(7-i);return {kind:'success',value:{...row,note:String(i).repeat(16384)}};})));
  results.forEach((r,i)=>expect(r).toEqual({kind:'success',value:{...row,note:String(i).repeat(16384)}}));
 }
 expect(poolStats.active).toBe(0);
});
test('renamed schemas and safe scalar values are data-driven; prototype keys remain own data',()=>{
 const other={version:1,digest:meta.digest,records:[{id:99,name:'Other',fields:['__proto__','text','number','flag','missing']}],entries:{'1':99}};
 const c=rowCodec(other),value=JSON.parse('{"__proto__":"safe","text":"é😀","number":9007199254740991,"flag":false}');
 expect(c.decode(99,c.encode(99,value))).toEqual(value);expect(Object.getPrototypeOf(value)).toBe(Object.prototype);
 expect(()=>c.encode(99,{...value,number:9007199254740992})).toThrow();
});
test('checked renamed source generates working binary ingress and egress',()=>{
 const p=JSON.parse(readFileSync('build/wasm-exp1/row-transport/mutation/renamed/projected/program.json','utf8'));
 const renamedModule=new WebAssembly.Module(readFileSync(import.meta.dir+'/compiler/build/renamed.wasm'));
 configureRows(renamedModule,JSON.parse(readFileSync(import.meta.dir+'/compiler/build/renamed-row-codec.json','utf8')));
 const op=p.declarations.find((d:any)=>d.name==='Record.read').semanticId;
 const {owner_id,...rest}=row;const value={...rest,account_key:owner_id,note:'é😀"\\'.repeat(2000)};
 expect(invokeSync(renamedModule,op,row.id,(capability,args:any)=>{
  expect(capability).toBe('storage.read');expect(args.entity).toBe('Record');expect(args.operation).toBe('Record.read');
  return {kind:'success',value};
 })).toEqual({kind:'success',value});
});
test('deterministic varied Unicode/control rows round-trip without aliasing',()=>{
 const alphabet=['x','\0','\u0001','\n','"','\\','é','中','😀','\ufeff'];let seed=12345;
 for(let n=0;n<400;n++){
  let note='';for(let i=0;i<n%100;i++){seed=(Math.imul(seed,1664525)+1013904223)>>>0;note+=alphabet[seed%alphabet.length];}
  const value={...row,note};const bytes=codec.encode(id,value).slice();codec.encode(id,{...row,note:'other'});
  expect(codec.decode(id,bytes)).toEqual(value);
 }
});
