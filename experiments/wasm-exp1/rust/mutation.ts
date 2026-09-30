import { invoke } from '../host/driver.ts';
const acceptance=await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const test=acceptance.probe_cases.find((c:any)=>c.id==='P15');
const results=[];
for(const [file,manifestFile,expect] of [
  ['build/probe.wasm','build/manifest.json',test.expect_before],
  ['build/mutated-probe.wasm','build/mutated/manifest.json',test.expect_after],
] as const) {
  const bytes=await Bun.file(new URL(file,import.meta.url)).arrayBuffer();
  const manifest=await Bun.file(new URL(manifestFile,import.meta.url)).json();
  const module=await WebAssembly.compile(bytes);
  let hostCalls=0;
  const result=await invoke(module,manifest.entry.semanticId,test.input,async()=>{
    hostCalls++; return {kind:'success',value:acceptance.seeds.Item[0]};
  });
  const pass=result.kind===expect.kind && (!('value'in expect)||result.value===expect.value) &&
    (!('host_calls'in expect)||hostCalls===expect.host_calls);
  results.push({file,checkedRevision:manifest.checkedRevision,sha256:new Bun.CryptoHasher('sha256').update(bytes).digest('hex'),result,hostCalls,pass});
}
console.log(JSON.stringify({id:'P15',results},null,2));
await Bun.write(new URL('build/mutation-results.json',import.meta.url),JSON.stringify({id:'P15',results},null,2)+'\n');
if(results.some(x=>!x.pass))process.exitCode=1;
