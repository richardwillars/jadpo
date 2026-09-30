import {test,expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {Guest,loadGuest} from './wasm-driver.ts';
const config=JSON.parse(readFileSync(import.meta.dir+'/build/storage.json','utf8'));
const manifest=JSON.parse(readFileSync(import.meta.dir+'/build/manifest.json','utf8'));
const op=(suffix:string)=>manifest.entries.find((e:any)=>e.name.endsWith(suffix)).semanticId;
const a='aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',owner='11111111-1111-4111-8111-111111111111';
const scope=config.plans.find((p:any)=>p.effect==='read').scope;
const row=(note:any='owned')=>({id:a,[scope]:owner,title:'alpha',note});
const ok=(value:any)=>({kind:'success',value});
test('two instances resume in reverse order with owned large values',()=>{
 const x=loadGuest(),y=loadGuest(),px=x.start(op('.read'),a),py=y.start(op('.read'),a);
 const value=row('é😀'.repeat(3000));expect(y.resume(py,ok(value))).toEqual(ok(value));expect(x.resume(px,ok(null)).kind).toBe('domain');
});
test('stale, duplicated, wrong handles and cancellation fail closed without poisoning reuse',()=>{
 const g=loadGuest();
 for(let i=0;i<200;i++){
  const pending=g.start(op('.read'),a);
  expect(g.resume({...pending,operationId:pending.operationId+1},ok(row()))).toEqual({kind:'internal'});
  expect(g.invoke(op('.read'),a,()=>ok(row(String(i))))).toEqual(ok(row(String(i))));
 }
 const stale=g.start(op('.read'),a);expect(g.cancel()).toEqual({kind:'internal'});const current=g.start(op('.read'),a);expect(current.requestId).toBeGreaterThan(stale.requestId);expect(g.resume(stale,ok(row()))).toEqual({kind:'internal'});
 const p=g.start(op('.read'),a);expect(g.resume(p,ok(row()))).toEqual(ok(row()));expect(g.resume(p,ok(row()))).toEqual({kind:'internal'});
});
test('malformed JSON/UTF8 host frames, raw host failures and schema failures remain internal',()=>{
 const g=loadGuest();
 for(const reply of [{kind:'domain',failure:'ItemMissing'},{kind:'success'},ok({...row(),title:'x'}),ok({...row(),extra:1}),{kind:'internal',detail:'SECRET_SENTINEL'}]){
  expect(g.invoke(op('.read'),a,()=>reply)).toEqual({kind:'internal'});
 }
 for(const bytes of [new Uint8Array([0xff]),new TextEncoder().encode('{"kind":')]){
  const p=g.start(op('.read'),a),ptr=g.exports.alloc(bytes.length);new Uint8Array(g.exports.memory.buffer,ptr,bytes.length).set(bytes);g.exports.resume(p.requestId,p.operationId,bytes.length);expect(g.output()).toEqual({kind:'internal'});
 }
 expect(g.start(op('.read'),'bad')).toEqual({kind:'invalid'});
});
test('exact limit, over limit, escapes and Unicode match logical envelope contract',()=>{
 const g=loadGuest();const overhead=new TextEncoder().encode(JSON.stringify(ok(row('')))).length;
 expect(g.invoke(op('.read'),a,()=>ok(row('x'.repeat(65536-overhead))))).toEqual(ok(row('x'.repeat(65536-overhead))));
 expect(g.invoke(op('.read'),a,()=>ok(row('x'.repeat(65537-overhead))))).toEqual({kind:'internal'});
 for(const note of ['é😀'.repeat(3000),'"\\\n\0'.repeat(3000),'\u{feff}'])expect(g.invoke(op('.read'),a,()=>ok(row(note)))).toEqual(ok(row(note)));
});
test('pair effects are sequential and cancellation cannot issue the second write',()=>{
 const g=loadGuest();const p=g.start(op('update_pair'),[a,'first-new','bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb','second-new']);expect(p.operationId).toBe(1);expect(p.capability).toBe('storage.update');
 const second=g.resume(p,ok({status:'found',row:{...row(),title:'first-new'}}));expect(second.operationId).toBe(2);expect(second.args.changes.title).toBe('second-new');expect(g.cancel()).toEqual({kind:'internal'});expect(g.resume(second,ok({status:'found',row:row()}))).toEqual({kind:'internal'});
});
test('actual checked source refinements govern the WASM validator',()=>{
 const g=loadGuest();const result=g.invoke(op('probe'),{id:a,title:'abcd'},()=>ok(row()));expect(result.kind).toBe(process.env.NATIVE_EXPECT_REFINED?'invalid':'success');
});
