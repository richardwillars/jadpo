import {test,expect} from 'bun:test';
import {compileDescriptor,createStorage} from './adapter.ts';
import {Database} from 'bun:sqlite';
import {bunSqlite} from '../optimization/cached-sqlite.ts';
import program from '../compiler/build/projected/program.json';
import spec from '../acceptance.json';
const fixture={operation:program.policy.operations[0],bindings:program.policy.bindings,entities:program.policy.entities};
const checker=compileDescriptor(fixture);
const reverse=(v:any):any=>Array.isArray(v)?v.map(reverse):v&&typeof v==='object'?Object.fromEntries(Object.entries(v).reverse().map(([k,x])=>[k,reverse(x)])):v;
test('compiled descriptor accepts reordered objects but rejects every changed leaf and shape',()=>{
 expect(checker(reverse(fixture))).toBe(true);
 const paths:any[][]=[];const walk=(v:any,p:any[]=[])=>{paths.push(p);if(v&&typeof v==='object')for(const [k,x] of Object.entries(v))walk(x,[...p,k]);};walk(fixture);
 let checked=0;
 for(const path of paths){
  for(const replacement of [null,true,42,'forged',[],{}]){
   const copy=structuredClone(fixture);let original:any=fixture;for(const k of path)original=original[k];if(JSON.stringify(original)===JSON.stringify(replacement))continue;
   if(!path.length){expect(checker(replacement)).toBe(false);checked++;continue;}
   let owner:any=copy;for(const k of path.slice(0,-1))owner=owner[k];owner[path.at(-1)!]=replacement;expect(checker(copy)).toBe(false);checked++;
  }
  if(path.length){const copy=structuredClone(fixture);let owner:any=copy;for(const k of path.slice(0,-1))owner=owner[k];delete owner[path.at(-1)!];expect(checker(copy)).toBe(false);checked++;}
 }
 expect(checker({...fixture,extra:null})).toBe(false);expect(checked).toBeGreaterThan(100);
});
test('long unconstrained Unicode remains valid while constrained Unicode lengths and forged rows reject',()=>{
 const db=new Database(':memory:');const storage=createStorage(program,bunSqlite(db));storage.setup();
 const rows=structuredClone(spec.seeds.Item);rows[0].note='😀'.repeat(8192);storage.resetFixture({Item:rows});expect(storage.snapshot().Item[0].note).toBe(rows[0].note);
 rows[0].title='😀😀😀';storage.resetFixture({Item:rows});
 rows[0].title='😀😀';expect(()=>storage.resetFixture({Item:rows})).toThrow();
 rows[0].title='😀'.repeat(13);expect(()=>storage.resetFixture({Item:rows})).toThrow();
 rows[0].title='alpha';(rows[0] as any).note=42;expect(()=>storage.resetFixture({Item:rows})).toThrow();db.close();
});
