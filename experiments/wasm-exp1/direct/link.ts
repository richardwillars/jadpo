import wabtFactory from '../tooling/node_modules/wabt/index.js';
const [runtimePath, outputPath] = process.argv.slice(2);
const wabt = await wabtFactory();
const runtimeBytes = await Bun.file(runtimePath).arrayBuffer();
const runtimeModule = await WebAssembly.compile(runtimeBytes);
if (WebAssembly.Module.imports(runtimeModule).length) throw new Error('Unexpected runtime imports');
const instance = await WebAssembly.instantiate(runtimeModule);
const base = (instance.exports.rt_constant_base as () => number)();
const parsed = wabt.readWasm(new Uint8Array(runtimeBytes), { readDebugNames: false });
const runtimeWat = parsed.toText({ foldExprs: false, inlineExport: false });
const exports = new Map<string,string>();
for (const match of runtimeWat.matchAll(/\(export "(rt_[^"]+)" \(func (\d+)\)\)/g)) exports.set(match[1], match[2]);
let application = await Bun.file(outputPath + '/application.wat').text();
application = application.replace(/@(rt_[a-z_]+)/g, (_, name) => {
  const index = exports.get(name);
  if (!index) throw new Error('Missing generic runtime helper ' + name);
  return index;
});
const constants = new Uint8Array(await Bun.file(outputPath + '/constants.bin').arrayBuffer());
if (constants.length > 65536) throw new Error('Constant capacity exceeded');
const data = Array.from(constants, byte => '\\' + byte.toString(16).padStart(2,'0')).join('');
// Existing function indices remain unchanged. Appended Wasm functions own all
// application control flow. Reserved runtime BSS owns the static data region.
const body = runtimeWat.trim().slice(0,-1).replace(/\s*\(export "rt_[^"]+" \(func \d+\)\)/g,'');
const linked = body + '\n' + application + `\n(data (i32.const ${base}) "${data}")\n)`;
await Bun.write(outputPath + '/linked.wat', linked);
const final = wabt.parseWat('direct-linked.wat',linked,{bulk_memory:false,simd:false,threads:false,reference_types:false,multi_value:false});
final.resolveNames(); final.validate();
const binary = final.toBinary({canonicalize_lebs:true,relocatable:false,write_debug_names:false}).buffer;
await WebAssembly.compile(binary);
await Bun.write(outputPath + '/probe.wasm',binary);
console.log(JSON.stringify({bytes:binary.length,constantBase:base,constantBytes:constants.length,exports:[...exports.keys()]}));
