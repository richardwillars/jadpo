import {invoke} from '../host/driver.ts';
const root=new URL('./build/variants/',import.meta.url);
const acceptance=await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const row=acceptance.seeds.Item[0], input=acceptance.probe_cases[0].input;
const results=[];
for(const name of ['renamed','branch']) {
 const module=await WebAssembly.compile(await Bun.file(new URL(name+'/module/probe.wasm',root)).arrayBuffer());
 const manifest=await Bun.file(new URL(name+'/module/manifest.json',root)).json();
 let hostCalls=0,metadata=false;
 const actual=await invoke(module,manifest.entry.semanticId,name==='renamed'?{id:input.id,caption:input.title}:input,async(capability,args:any)=>{
  hostCalls++;
  if(name==='renamed') {
   metadata=capability==='storage.read'&&args.entity==='Widget'&&args.operation==='Widget.read'&&
    args.policy.bindings[0].field==='holder_id'&&args.predicate.value===input.id;
   return {kind:'success',value:{id:row.id,holder_id:row.owner_id,caption:row.title,note:row.note}};
  }
  metadata=capability==='storage.read'&&args.entity==='Item';
  return {kind:'success',value:row};
 });
 const expected=name==='renamed'?row.title:input.title;
 results.push({case:name,pass:hostCalls===1&&metadata&&actual.kind==='success'&&actual.value===expected,actual,hostCalls,metadata,checkedRevision:manifest.checkedRevision});
}
await Bun.write(new URL('../source-variant-results.json',root),JSON.stringify(results,null,2)+'\n');
console.log(JSON.stringify(results));
if(results.some(x=>!x.pass))process.exitCode=1;
