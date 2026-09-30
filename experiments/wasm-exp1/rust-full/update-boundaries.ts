import {invoke} from '../host/driver.ts';
const manifest=await Bun.file(new URL('build/manifest.json',import.meta.url)).json();
const module=await WebAssembly.compile(await Bun.file(new URL('build/probe.wasm',import.meta.url)).arrayBuffer());
const spec=await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const entry=manifest.entries.find((e:any)=>e.name==='Item.change');
const row=spec.seeds.Item[0], valid=[row.id,{title:'renamed'}];
const checks=[
 {label:'object-instead-of-array',input:{id:row.id,changes:{title:'renamed'}},kind:'invalid'},
 {label:'extra-argument',input:[...valid,'extra'],kind:'invalid'},
 {label:'missing-argument',input:[row.id],kind:'invalid'},
 {label:'null-nonnullable-argument',input:[row.id,null],kind:'invalid'},
 {label:'unknown-patch-field',input:[row.id,{owner_id:spec.seeds.Item[2].owner_id}],kind:'invalid'},
 ...[
  {status:'found'}, {status:'found',row:{...row,title:42}},
  {status:'found',row,extra:true}, {status:'missing',row},
  {status:'conflict',detail:'WASM_EXP1_SECRET_SENTINEL'}, {status:'unknown'},
 ].map(reply=>({label:'malformed-update-result',input:valid,reply,kind:'internal'})),
];
const results=[];
for(const c of checks){let calls=0;const actual=await invoke(module,entry.semanticId,c.input,async()=>{calls++;return {kind:'success',value:(c as any).reply};});
 const pass=actual.kind===c.kind&&calls===(c.kind==='invalid'?0:1)&&(c.kind!=='internal'||actual.operation===entry.semanticId)&&!JSON.stringify(actual).includes('WASM_EXP1_SECRET_SENTINEL');
 results.push({label:c.label,pass,actual,hostCalls:calls});}
await Bun.write(new URL('build/update-boundaries.json',import.meta.url),JSON.stringify(results,null,2)+'\n');
console.log(JSON.stringify({passed:results.filter(r=>r.pass).length,failed:results.filter(r=>!r.pass).length}));
if(results.some(r=>!r.pass))process.exitCode=1;
