// Fresh-process measurement harness; does not alter generated application logic.
import { invoke } from '../host/driver.ts';
const scriptStart = performance.now();
const spec = await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const manifest = await Bun.file(new URL('build/manifest.json',import.meta.url)).json();
const bytes = await Bun.file(new URL('build/probe.wasm',import.meta.url)).arrayBuffer();
const readFinished = performance.now();
const module = await WebAssembly.compile(bytes);
const compiled = performance.now();
let instantiateMs = 0;
let memory: WebAssembly.Memory | undefined;
const originalInstantiate = WebAssembly.instantiate;
// Observe the exact instance created by the common driver, rather than creating
// an extra measuring instance. Single invocation; no process/module reuse.
(WebAssembly as any).instantiate = async (...args: any[]) => {
  const started = performance.now();
  const result = await (originalInstantiate as any)(...args);
  instantiateMs += performance.now() - started;
  memory = result.exports.memory;
  return result;
};
let hostCalls=0;
const invokeStart=performance.now();
const result = await invoke(module,manifest.entry.semanticId,spec.probe_cases[0].input,async(capability,args:any)=>{
  hostCalls++;
  if(capability!=='storage.read'||args.entity!=='Item'||args.freshness!=='authoritative')
    throw new Error('Unexpected checked storage capability');
  return {kind:'success',value:spec.seeds.Item[0]};
});
const invokeEnd=performance.now();
const pass=result.kind==='success'&&result.value==='alpha'&&hostCalls===1;
console.log(JSON.stringify({pass,result,hostCalls,
  artifactSha256:new Bun.CryptoHasher('sha256').update(bytes).digest('hex'),
  checkedRevision:manifest.checkedRevision,
  scriptReadSetupMs:readFinished-scriptStart,wasmCompileMs:compiled-readFinished,
  instantiateMs,invokeIncludingInstantiateMs:invokeEnd-invokeStart,
  scriptTotalMs:invokeEnd-scriptStart,
  residentSetBytes:process.memoryUsage().rss,
  runtimeResourceUsage:process.resourceUsage(),
  finalLinearMemoryPages:memory!.buffer.byteLength/65536,
  scope:'Fresh Bun process, compile+instantiate+one frozen probe; fixture host in memory, no I/O or artificial delay.'}));
if(!pass)process.exitCode=1;
