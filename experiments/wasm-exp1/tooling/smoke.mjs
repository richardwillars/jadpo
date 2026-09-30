// Host packaging preflight only. This is not a Jadpo compilation route.
import wabtFactory from "wabt";
import { mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const out = new URL("../../../build/wasm-exp1/tools/smoke/", import.meta.url);
await mkdir(out, { recursive: true });
const wat = `(module
  (import "host" "identity" (func $identity (param i32) (result i32)))
  (func (export "run") (param i32) (result i32)
    local.get 0
    call $identity))`;
const wabt = await wabtFactory();
const parsed = wabt.parseWat("host-smoke.wat", wat);
parsed.resolveNames();
parsed.validate();
const { buffer } = parsed.toBinary({ canonicalize_lebs: true, write_debug_names: false });
parsed.destroy();
const module = new WebAssembly.Module(buffer);
const instance = new WebAssembly.Instance(module, { host: { identity: (value) => value } });
if (instance.exports.run(42) !== 42) throw new Error("Wasm smoke result mismatch");
await writeFile(new URL("module.wasm", out), buffer);
await writeFile(new URL("host-smoke.wat", out), wat + "\n");
await writeFile(new URL("worker.js", out), `import module from "./module.wasm";
const instance = await WebAssembly.instantiate(module, { host: { identity: (value) => value } });
export default { fetch() { return Response.json({ hostSmoke: true, result: instance.exports.run(42) }); } };
`);
await writeFile(new URL("wrangler.jsonc", out), JSON.stringify({
  "$schema": "../../../../experiments/wasm-exp1/tooling/node_modules/wrangler/config-schema.json",
  name: "jadpo-wasm-exp1-host-smoke",
  main: "worker.js",
  compatibility_date: "2026-09-30",
  workers_dev: false,
  preview_urls: false,
}, null, 2) + "\n");
const result = {
  scope: "host packaging only; not compiler route evidence",
  wasm_bytes: buffer.byteLength,
  wasm_sha256: createHash("sha256").update(buffer).digest("hex"),
  imports: WebAssembly.Module.imports(module),
  exports: WebAssembly.Module.exports(module),
  node_result: instance.exports.run(42),
};
await writeFile(new URL("module-inspection.json", out), JSON.stringify(result, null, 2) + "\n");
console.log(JSON.stringify({ ...result, output: fileURLToPath(out) }, null, 2));
