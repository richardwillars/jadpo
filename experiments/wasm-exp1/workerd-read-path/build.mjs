// Node is development tooling only; the output executes in workerd, without Bun.
import {build} from '../tooling/node_modules/esbuild/lib/main.js';
import {readFileSync,writeFileSync,mkdirSync,copyFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(import.meta.dirname,'../../..'),out=root+'/build/wasm-exp1/workerd-read-path/bundle';mkdirSync(out,{recursive:true});
const original=root+'/build/wasm-exp1/baseline/generated/bun/target/app.ts';
const result=await build({entryPoints:[import.meta.dirname+'/worker.ts'],outfile:out+'/worker.mjs',bundle:true,minifySyntax:true,format:'esm',platform:'neutral',target:'es2022',metafile:true,external:['cloudflare:workers','./candidate.wasm','./typed.wasm','./previous.wasm'],define:{'import.meta.main':'false','Bun.env.JADPO_DEBUG_TARGET_STACKS':'undefined','Bun.version':'"unused-on-workerd"','process.versions.icu':'"workerd"'},plugins:[{name:'portable-generated-read-port',setup(b){b.onResolve({filter:/^\.\/persistence\.ts$/},args=>args.importer===original?{path:import.meta.dirname+'/persistence-port.ts'}:undefined);}}]});
const bundle=readFileSync(out+'/worker.mjs','utf8');if(/\bBun\s*\.|\bbun:|\bprocess\s*\.|\bnode:/.test(bundle))throw Error('Unexpected runtime dependency');
for(const name of ['candidate','typed','previous'])copyFileSync(root+`/experiments/wasm-exp1/${name==='previous'?'typed-values':'read-path'}/compiler/build/application.wasm`,out+'/'+name+'.wasm');
const config={name:'jadpo-local-workerd-read-path',main:'worker.mjs',compatibility_date:'2026-09-30',workers_dev:false,preview_urls:false,durable_objects:{bindings:[{name:'READ_AUTHORITY',class_name:'ReadAuthority'}]},migrations:[{tag:'local-v1',new_sqlite_classes:['ReadAuthority']}],vars:{EXPERIMENT_TOKEN:'local-placeholder-replaced-by-runner'},observability:{enabled:true,head_sampling_rate:0}};
writeFileSync(out+'/wrangler.jsonc',JSON.stringify(config,null,2)+'\n');
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
writeFileSync(out+'/manifest.json',JSON.stringify({scope:'Local-only workerd bundle. No deployment. Same compiler-produced JS read functions/validators with explicit read-only authority port; Bun CLI and unused runtime provenance constants removed at bundle time.',hashes:{generatedJs:hash(original),worker:hash(out+'/worker.mjs'),candidate:hash(out+'/candidate.wasm'),previous:hash(out+'/previous.wasm')},noBunOrNodeRuntimeDependency:true,bundleInputs:result.metafile.inputs},null,2)+'\n');
console.log(out);
