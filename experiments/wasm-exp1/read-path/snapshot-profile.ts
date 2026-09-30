// Bounded diagnostic: distinguish snapshot creation from JSON serialization.
import {readFileSync,writeFileSync} from 'node:fs';
import {rowCodec} from './codec.ts';
const meta=JSON.parse(readFileSync(import.meta.dir+'/compiler/build/row-codec.json','utf8')),codec=rowCodec(meta),schema=codec.schema('Item')!;
const row={...JSON.parse(readFileSync('experiments/wasm-exp1/acceptance.json','utf8')).seeds.Item[0],note:'x'.repeat(16384)};
const frozen=Object.freeze(codec.snapshot(schema,row)),plain={...row};
const cases:Record<string,()=>any>={snapshot:()=>codec.snapshot(schema,row),plainClone:()=>({...row}),stringifyOriginal:()=>JSON.stringify({kind:'success',value:row}),stringifyFrozen:()=>JSON.stringify({kind:'success',value:frozen}),stringifyPlain:()=>JSON.stringify({kind:'success',value:plain}),binaryEncode:()=>codec.encode(schema,row)};
const results:any[]=[];let sink:any;
for(let run=0;run<5;run++)for(const [name,call] of Object.entries(cases)){let count=0;const start=performance.now(),end=start+200;while(performance.now()<end){sink=call();count++;}results.push({run:run+1,name,meanUs:(performance.now()-start)*1000/count});}
if(!sink)throw Error('profile not executed');writeFileSync('build/wasm-exp1/read-path/snapshot-profile.json',JSON.stringify({scope:'Isolated component diagnostic, not full-request performance',results},null,2)+'\n');console.log(JSON.stringify(results));
