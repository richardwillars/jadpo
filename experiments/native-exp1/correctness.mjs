import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {launch,spec,a,b,c,out} from './harness.mjs';
const evidence=[];
const success=value=>({kind:'success',value}),domain=failure=>({kind:'domain',failure});
for(const journal of ['WAL','DELETE'])for(const target of ['native','bun']){
 const s=await launch(target,journal,true);const cases=[];
 async function check(name,operation,input,expected,principal='owner',unchanged=true){
  const before=await s.control('snapshot');const r=await s.send('call',{operation,input,principal});assert.deepEqual(r.body,expected,`${target} ${name}`);assert.equal(r.status,expected.kind==='success'?200:expected.kind==='domain'?422:expected.kind==='invalid'?400:500);
  if(unchanged)assert.deepEqual(await s.control('snapshot'),before,name+' snapshot');cases.push({name,result:r.body});
 }
 try{
  await check('fresh owned row','Item.read',a.id,success(a));
  await check('field extraction','Item.read_title',a.id,success(a.title));
  await check('outsider read','Item.read',a.id,domain('ItemMissing'),'other');
  await check('missing read','Item.read',spec.seeds.missing_item_id,domain('ItemMissing'));
  await check('authored recovery','probe',{id:spec.seeds.missing_item_id,title:'fallback'},success('fallback'));
  for(const input of [{id:'bad',title:'valid'},{id:a.id,title:'x'},{id:a.id,title:'abcdefghijklm'},{id:a.id,title:'valid',extra:1},{id:a.id,title:'valid',note:42}])await check('invalid probe '+JSON.stringify(input),'probe',input,{kind:'invalid'});
  for(const input of [[a.id,{owner_id:c.owner_id}],[a.id,{title:'x'}],[a.id,{note:9}],[a.id,{},1]])await check('invalid patch '+JSON.stringify(input),'Item.change',input,{kind:'invalid'});
  await check('Unicode scalar title','probe',{id:spec.seeds.missing_item_id,title:'😀😀😀'},success('😀😀😀'));
  await check('empty patch','Item.change',[a.id,{}],domain('EmptyPatch'));
  await check('outsider write','Item.change',[a.id,{note:'forbidden'}],domain('ItemMissing'),'other');
  await check('conflict','Item.rename',[a.id,b.title],domain('ItemConflict'));
  await check('missing pair rollback','update_pair',[a.id,'first-new',spec.seeds.missing_item_id,'second-new'],domain('ItemMissing'));
  await check('conflict pair rollback','update_pair',[a.id,'first-new',b.id,c.title],domain('ItemConflict'));
  await check('omitted note','Item.change',[a.id,{title:'renamed'}],success({...a,title:'renamed'}),'owner',false);
  await check('null note','Item.change',[a.id,{note:null}],success({...a,title:'renamed',note:null}),'owner',false);
  await check('empty text note','Item.change',[a.id,{note:''}],success({...a,title:'renamed',note:''}),'owner',false);
  await s.control('reset');
  await check('pair commit','update_pair',[a.id,'first-new',b.id,'second-new'],success({...b,title:'second-new'}),'owner',false);
  assert.deepEqual(await s.control('snapshot'),[{...a,title:'first-new'},{...b,title:'second-new'},c]);
  await s.control('reset');
  await s.control('mutate',{sql:`UPDATE entity_Item SET owner_id='${c.owner_id}',note='changed' WHERE id='${a.id}'`});
  await check('ownership freshness old owner','Item.read',a.id,domain('ItemMissing'));
  await check('ownership freshness new owner','Item.read',a.id,success({...a,owner_id:c.owner_id,note:'changed'}),'other');
  await s.control('reset');await s.control('mutate',{sql:`UPDATE entity_Item SET title='x' WHERE id='${a.id}'`});
  await check('corrupt stored title','probe',{id:a.id,title:'fallback'},{kind:'internal'});
  await s.control('reset');
  await s.control('mutate',{sql:`CREATE TRIGGER fail_second BEFORE UPDATE ON entity_Item WHEN OLD.id='${b.id}' BEGIN SELECT RAISE(ABORT,'WASM_EXP1_SECRET_SENTINEL'); END`});
  await check('trigger constraint classification (known target difference)','update_pair',[a.id,'first-new',b.id,'second-new'],target==='bun'?domain('ItemConflict'):{kind:'internal'});
  await s.control('mutate',{sql:'DROP TRIGGER fail_second'});
  await s.control('mutate',{sql:`CREATE TRIGGER fail_second BEFORE UPDATE ON entity_Item WHEN OLD.id='${b.id}' BEGIN SELECT abs(-9223372036854775808); END`});
  await check('driver error rolls back first write','update_pair',[a.id,'first-new',b.id,'second-new'],{kind:'internal'});
  await s.control('mutate',{sql:'DROP TRIGGER fail_second'});
  for(const note of ['x'.repeat(256),'x'.repeat(4096),'x'.repeat(16384),'x'.repeat(49152),'é😀'.repeat(3000),'x'.repeat(8192)+'\n"\\'.repeat(500)]){await s.control('reset',{note});await check('large/escaped '+Buffer.byteLength(note),'Item.read',a.id,success({...a,note}));await check('large title extraction','Item.read_title',a.id,success(a.title));}
  await s.control('reset');
  assert.deepEqual((await s.send('call','{"operation":',true)).body,{kind:'invalid'});cases.push({name:'malformed input',result:'pass'});
  await s.control('trace',{enabled:true});await check('SQL read trace','Item.read',a.id,success(a));await check('SQL write trace','Item.rename',[a.id,'renamed'],success({...a,title:'renamed'}),'owner',false);const stats=await s.control('stats');
  for(let i=0;i<200;i++){const principal=i%2?'owner':'other';await check('reuse '+i,'Item.read',a.id,principal==='owner'?success({...a,title:'renamed'}):domain('ItemMissing'),principal);}
  evidence.push({target,journal,pragmas:s.info,cases,sql:stats.trace,stderr:s.stderr()});
 }finally{await s.stop();writeFileSync(out+'/correctness.json',JSON.stringify(evidence,null,2)+'\n');}
}
console.log(JSON.stringify({servers:evidence.length,cases:evidence.reduce((n,x)=>n+x.cases.length,0)}));
