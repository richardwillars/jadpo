import { invoke } from '../host/driver.ts';
const root = new URL('../', import.meta.url);
const cases = await Bun.file(new URL('acceptance.json', root)).json();
const bytes = await Bun.file(new URL('build/probe.wasm', import.meta.url)).arrayBuffer();
const module = await WebAssembly.compile(bytes);
const manifest = await Bun.file(new URL('build/manifest.json', import.meta.url)).json();
const results = [];
for (const c of cases.probe_cases.filter((x: any) => Number(x.id.slice(1)) <= 11)) {
  let calls = 0;
  const result = await invoke(module, manifest.entry.semanticId, c.input, async (capability, args: any) => {
    calls++;
    if (capability !== 'storage.read' || args.freshness !== 'authoritative' ||
        !args.policy.operation.obligations.length || !args.policy.bindings.length)
      throw new Error('missing checked policy plan');
    await Bun.sleep(c.host?.delay_ms ?? 0);
    if (c.host?.fault) throw new Error(c.host.fault);
    return {kind:'success',value:c.host?.read_missing ? null : c.host?.read_row};
  });
  const pass = result.kind === c.expect.kind &&
    (!('value' in c.expect) || result.value === c.expect.value) &&
    (!('host_calls' in c.expect) || calls === c.expect.host_calls) &&
    !JSON.stringify(result).includes('WASM_EXP1_SECRET_SENTINEL');
  results.push({id:c.id,pass,result,hostCalls:calls});
}
const output = {route:'rust',artifactBytes:bytes.byteLength,imports:WebAssembly.Module.imports(module),
  exports:WebAssembly.Module.exports(module),results};
console.log(JSON.stringify(output,null,2));
await Bun.write(new URL('build/smoke-results.json',import.meta.url),JSON.stringify(output,null,2)+'\n');
if (results.some(r=>!r.pass)) process.exitCode=1;
