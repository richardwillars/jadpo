import {invoke} from '../host/driver.ts';
const path=process.argv[2];
const manifest=await Bun.file(`${path}/crate/manifest.json`).json();
const module=await WebAssembly.compile(await Bun.file(`${path}/module.wasm`).arrayBuffer());
const acceptance=await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const entry=manifest.entries[0];
const multiple=entry.parameters.length>1;
const results=[];
for(const value of [null,'valid',42,'x']) {
 let calls=0;
 const input=multiple?[acceptance.probe_cases[0].input,value]:value;
 const result=await invoke(module,entry.semanticId,input,async()=>{calls++;return {kind:'success',value:acceptance.seeds.Item[0]};});
 const valid=value===null||value==='valid';
 const pass=valid?result.kind==='success'&&result.value===(multiple?'alpha':value)&&calls===(multiple?1:0):result.kind==='invalid'&&calls===0;
 results.push({input,result,hostCalls:calls,pass});
}
console.log(JSON.stringify({entry:entry.name,checkedRevision:manifest.checkedRevision,results}));
if(results.some(r=>!r.pass))process.exitCode=1;
