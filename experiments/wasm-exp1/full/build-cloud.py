#!/usr/bin/env python3
"""Package explicit compiler artifacts and trusted fixed-case glue; never deploy."""
import argparse, hashlib, json, pathlib, shutil, subprocess
ROOT = pathlib.Path(__file__).resolve().parents[3]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('module', type=pathlib.Path)
p.add_argument('output', type=pathlib.Path)
p.add_argument('--program', type=pathlib.Path, default=ROOT/'experiments/wasm-exp1/compiler/build/projected/program.json')
p.add_argument('--mutated', type=pathlib.Path)
p.add_argument('--probe-only', action='store_true')
a = p.parse_args()
out = a.output.resolve()
allowed = (ROOT/'build/wasm-exp1').resolve()
if not out.is_relative_to(allowed): raise SystemExit('Output must be under ignored build/wasm-exp1')
# Validate the capability surface before writing a publishable package.
for artifact in [a.module] + ([a.mutated] if a.mutated else []):
    subprocess.run(['node','--input-type=module','-e','import fs from "node:fs";const m=new WebAssembly.Module(fs.readFileSync(process.argv[1]));if(WebAssembly.Module.imports(m).length)throw Error("Unapproved Wasm host import");const e=new Map(WebAssembly.Module.exports(m).map(x=>[x.name,x.kind]));for(const n of ["alloc","start","resume","result_ptr","result_len"])if(e.get(n)!=="function")throw Error("Missing ABI export");if(e.get("memory")!=="memory")throw Error("Missing ABI memory");',str(artifact.resolve())],check=True)
out.mkdir(parents=True, exist_ok=True)
base = ROOT/'experiments/wasm-exp1'
for relative in ['full/host.ts','full/suite.ts','host/driver.ts','host/probe.ts','storage/adapter.ts','storage/cloudflare-sqlite.ts']:
    target=out/relative; target.parent.mkdir(parents=True,exist_ok=True); shutil.copyfile(base/relative,target)
for original,name in [(a.module,'application.wasm'),(a.program,'program.json'),(base/'acceptance.json','acceptance.json')]: shutil.copyfile(original,out/name)
if a.mutated: shutil.copyfile(a.mutated,out/'mutated.wasm')
wat='''(module (memory (export "memory") 2 128)
(func (export "alloc") (param i32) (result i32) i32.const 1024)
(func (export "start") (param i32 i32 i32) (result i32) unreachable)
(func (export "resume") (param i32 i32 i32 i32) (result i32) unreachable)
(func (export "result_ptr") (result i32) i32.const 0)
(func (export "result_len") (result i32) i32.const 0))'''
(out/'abi-trap.wat').write_text(wat)
encoder=base/'tooling/node_modules/.bin/wat2wasm'
subprocess.run([str(encoder),str(out/'abi-trap.wat'),'-o',str(out/'abi-trap.wasm')],check=True)
subprocess.run(['node','--input-type=module','-e','import fs from "node:fs";const module=new WebAssembly.Module(fs.readFileSync(process.argv[1]));try{new WebAssembly.Instance(module).exports.start(0,0,0);throw Error("not trapped");}catch(e){if(!(e instanceof WebAssembly.RuntimeError))throw e;}',str(out/'abi-trap.wasm')],check=True)
worker='''// Generated trusted experiment orchestration. Fixed synthetic cases only.
import { DurableObject } from "cloudflare:workers";
import application from "./application.wasm";
import trapModule from "./abi-trap.wasm";
MUTATED_IMPORT
import program from "./program.json";
import spec from "./acceptance.json";
import metadata from "./package-evidence.json";
import { createSliceHost } from "./full/host.ts";
import { runFullSuite } from "./full/suite.ts";
import { runProbeSuite } from "./host/probe.ts";
import { cloudflareSqlite } from "./storage/cloudflare-sqlite.ts";
export class SliceAuthority extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.host=createSliceHost(application,program,spec,cloudflareSqlite(ctx.storage),{
      trapModule,
      emitEvent:event=>console.error(JSON.stringify(event)),
      reopen:async()=>{throw new Error("Reopen must cross the outer RPC boundary");},
    });
  }
  reset(){return this.host.reset();}
  snapshot(){return this.host.snapshot();}
  invoke(operation,input,principal,control){return this.host.invoke(operation,input,principal,control);}
}
export default {
  async fetch(request,env) {
    if(request.method!=="GET"||new URL(request.url).pathname!=="/report")return new Response("Not found",{status:404});
    // One fresh authority per report; every principal and input comes from frozen spec.
    const reportId=crypto.randomUUID();
    const stub=env.SLICE_AUTHORITY.getByName(`synthetic-report:${reportId}`);
    const host={
      reset:()=>stub.reset(),snapshot:()=>stub.snapshot(),
      reopen:()=>stub.snapshot(), // Deliberately a new RPC, not same-call storage reuse.
      async invoke(operation,input,principal,control={}) {
        if(control.delay_ms)await new Promise(resolve=>setTimeout(resolve,control.delay_ms));
        const {delay_ms,...inside}=control;
        return stub.invoke(operation,input,principal,inside);
      },
    };
    const probe=await runProbeSuite(application,spec,MUTATED_OPTIONS);
    const full=FULL_EXPRESSION;
    return Response.json({reportId,metadata,probe,full},{headers:{"cache-control":"no-store"}});
  },
};
'''
worker=worker.replace('MUTATED_IMPORT','import mutatedModule from "./mutated.wasm";' if a.mutated else '')
worker=worker.replace('MUTATED_OPTIONS','{mutatedModule,projection:program}' if a.mutated else '{projection:program}')
worker=worker.replace('FULL_EXPRESSION',"{status:'not_run',reason:'Explicit probe-only package; full slice unavailable',results:spec.full_slice_cases.map(c=>({id:c.id,status:'not_run'}))}" if a.probe_only else 'await runFullSuite(host,spec,program)')
(out/'worker.js').write_text(worker)
config={'name':'jadpo-wasm-exp1-full-20260930','main':'worker.js','compatibility_date':'2026-09-30','workers_dev':True,'preview_urls':False,'durable_objects':{'bindings':[{'name':'SLICE_AUTHORITY','class_name':'SliceAuthority'}]},'migrations':[{'tag':'wasm-exp1-v1','new_sqlite_classes':['SliceAuthority']}],'observability':{'enabled':True,'head_sampling_rate':1}}
(out/'wrangler.jsonc').write_text(json.dumps(config,indent=2)+'\n')
program=json.loads(a.program.read_text())
sha=lambda f:hashlib.sha256(pathlib.Path(f).read_bytes()).hexdigest()
evidence={'status':'packaged_not_deployed','artifactScope':'probe_only' if a.probe_only else 'explicit_full_slice_candidate_not_verified','artifactSha256':sha(out/'application.wasm'),'projectionSha256':sha(out/'program.json'),'checkedRevision':program['checkedRevision'],'acceptanceSha256':sha(out/'acceptance.json'),'trapFixture':{'scope':'handwritten actual unreachable ABI host-fault fixture; not application compiler output','sha256':sha(out/'abi-trap.wasm'),'actualRuntimeTrapVerified':True},'mutatedArtifactSha256':sha(out/'mutated.wasm') if a.mutated else None,'fixedSyntheticGETReportOnly':True,'fullA16Reopen':'separate snapshot RPC after invocation','capabilityEvidence':'Package only; no full-slice runtime pass follows from building it.'}
(out/'package-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n')
print(json.dumps({'status':'packaged_not_deployed','output':str(out),'artifactSha256':evidence['artifactSha256'],'scope':evidence['artifactScope']}))
