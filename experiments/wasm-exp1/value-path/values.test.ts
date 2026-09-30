import {test,expect} from 'bun:test';
import {environment,originalRows} from './environment.ts';
const a=originalRows[0];
test('all build variants preserve large, escaped and Unicode values and multi-argument updates',async()=>{
 const values=[null,'x'.repeat(16384),'😀'.repeat(4096),'"\\\n\r\t\u0000'.repeat(1500),'é中😀'.repeat(1500),''];
 for(const target of ['previous','speed','ownership','combined']){
  const e=await environment(target,':memory:');e.reset();
  for(const note of values){
   const changed=await e.call('Item.change',[a.id,{note}]);expect(changed).toEqual({kind:'success',value:{...a,note}});
   expect(await e.call('Item.read',a.id)).toEqual(changed);
   expect(await e.call('Item.read',a.id,'other')).toEqual({kind:'domain',failure:'ItemMissing'});
  }
  expect(await e.call('Item.change',[a.id,{}])).toEqual({kind:'domain',failure:'EmptyPatch'});
  expect(await e.call('Item.change',[a.id,{note:'ok'},'extra'])).toEqual({kind:'invalid'});
  expect(await e.call('Item.change',[a.id])).toEqual({kind:'invalid'});
  e.close();
 }
});
