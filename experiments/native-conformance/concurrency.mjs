import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {launch,a,b,c,out} from './harness.mjs';
const results=[];
for(const target of ['native','bun','wasm']){
 const s=await launch(target);let verified=0;
 try{
  await s.control('reset');
  await Promise.all(Array.from({length:16},async(_,lane)=>{for(let i=0;i<50;i++){
   const other=lane%2===1;const result=await s.control('call',{operation:'Item.read',input:a.id,principal:other?'other':'owner'});assert.deepEqual(result,other?{kind:'domain',failure:'ItemMissing'}:{kind:'success',value:a});verified++;
  }}));
  await Promise.all(Array.from({length:16},async(_,lane)=>{for(let i=0;i<20;i++){
   const denied=i%4===0;const input=[a.id,`one${lane}-${i%2}`,b.id,denied?c.title:`two${lane}-${i%2}`];const result=await s.control('call',{operation:'update_pair',input});assert.deepEqual(result,denied?{kind:'domain',failure:'ItemConflict'}:{kind:'success',value:{...b,title:input[3]}});verified++;
  }}));
  const snapshot=await s.control('snapshot');assert.equal(snapshot[1].title,'two'+snapshot[0].title.slice(3));assert.deepEqual({...snapshot[0],title:a.title},a);assert.deepEqual({...snapshot[1],title:b.title},b);assert.deepEqual(snapshot[2],c);
  results.push({target,lanes:16,verified,snapshotVerified:true});
 }finally{await s.stop();writeFileSync(out+'/concurrency.json',JSON.stringify(results,null,2)+'\n');}
}
console.log(JSON.stringify(results));
