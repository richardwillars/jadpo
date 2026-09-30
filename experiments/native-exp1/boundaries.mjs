// Adversarial checks kept separate from the frozen timing harness.
import assert from 'node:assert/strict';
import {isDeepStrictEqual} from 'node:util';
import {writeFileSync} from 'node:fs';
import {launch,a,b,c,out} from './harness.mjs';
const evidence=[];
for(const target of ['native','bun']){
 const s=await launch(target);const cases=[];
 try{
  const envelopeBytes=Buffer.byteLength(JSON.stringify({kind:'success',value:{...a,note:''}}));
  for(const length of [65536-envelopeBytes,65537-envelopeBytes]){
   await s.control('reset');await s.control('mutate',{sql:`UPDATE entity_Item SET note=replace(hex(zeroblob(${length})),'00','x') WHERE id='${a.id}'`});
   const actual=await s.control('call',{operation:'Item.read',input:a.id});
   assert.deepEqual(actual,length===65536-envelopeBytes?{kind:'success',value:{...a,note:'x'.repeat(length)}}:{kind:'internal'});
   cases.push({name:'exact and over logical row envelope',envelopeBytes:length+envelopeBytes,kind:actual.kind});
  }
  await s.control('reset');await s.control('mutate',{sql:`UPDATE entity_Item SET note=x'ff' WHERE id='${a.id}'`});
  assert.deepEqual(await s.control('call',{operation:'Item.read',input:a.id}),{kind:'internal'});cases.push({name:'invalid stored blob',kind:'internal'});
  await s.control('reset');const note='x'.repeat(65537-envelopeBytes),body={operation:'Item.change',input:[a.id,{note}]};assert(Buffer.byteLength(JSON.stringify(body))<=65536);
  const actual=await s.control('call',body),snapshot=await s.control('snapshot');assert.deepEqual(actual,{kind:'internal'});
  const rolledBack=isDeepStrictEqual(snapshot,[a,b,c]);
  // Deliberately retain this observation even if the existing Bun wrapper applies
  // its response limit after commit. It cannot be called transaction parity.
  if(target==='native')assert.deepEqual(snapshot,[a,b,c]);
  else assert.deepEqual(snapshot,[{...a,note},b,c]);
  cases.push({name:'oversized write result',kind:actual.kind,rolledBack,targetDifference:true});
  assert.deepEqual(await s.control('call',{operation:'probe',input:{id:c.id,title:'fallback'}}),{kind:'success',value:'fallback'});
  evidence.push({target,cases,stderr:s.stderr()});
 }finally{await s.stop();writeFileSync(out+'/boundaries.json',JSON.stringify(evidence,null,2)+'\n');}
}
console.log(JSON.stringify(evidence));
