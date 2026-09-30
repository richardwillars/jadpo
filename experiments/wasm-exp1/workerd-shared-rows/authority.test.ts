import {test,expect} from 'bun:test';
import {environment,originalRows,spec} from './environment.ts';
test('large-row references preserve fresh reads and ownership checks on real SQLite',async()=>{
 for(const target of ['reference','combined','compact-reference','compact-combined']){
  const env=await environment(target,':memory:');
  try{
   env.reset(true);
   expect(await env.call('Item.read',originalRows[0].id,'owner')).toEqual({kind:'success',value:{...originalRows[0],note:'x'.repeat(16384)}});
   expect(await env.call('Item.read',originalRows[0].id,'other')).toEqual({kind:'domain',failure:'ItemMissing'});
   // Change both content and ownership between calls. A stale row or cached
   // permission would wrongly reveal the new value to the old owner.
   env.db.query('UPDATE entity_Item SET note=?,owner_id=? WHERE id=?').run('y'.repeat(16384),spec.seeds.principals.other.id,originalRows[0].id);
   expect(await env.call('Item.read',originalRows[0].id,'owner')).toEqual({kind:'domain',failure:'ItemMissing'});
   expect(await env.call('Item.read',originalRows[0].id,'other')).toEqual({kind:'success',value:{...originalRows[0],owner_id:spec.seeds.principals.other.id,note:'y'.repeat(16384)}});
   expect(await env.call('Item.read_title',originalRows[0].id,'other')).toEqual({kind:'success',value:originalRows[0].title});
  }finally{env.close();}
 }
});
