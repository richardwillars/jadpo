// Explicit artifact runner: does not claim a probe-only module implements the full slice.
import {Database} from "bun:sqlite";
import {mkdtemp,mkdir,readFile,writeFile} from "node:fs/promises";
import {resolve,join,basename} from "node:path";
import {createHash} from "node:crypto";
import {createStorage} from "../boundary-http/adapter.ts";
import {bunSqlite} from "../optimization/cached-sqlite.ts";
import {poolStats,configureRows} from "./driver.ts";
import {createSliceHost} from "./cached-host.ts";
import {runFullSuite} from "../full/suite.ts";
import {runProbeSuite} from "./probe.ts";
import wabtFactory from "../tooling/node_modules/wabt/index.js";
const root=resolve(import.meta.dir,"../../..");
const outputRoot=join(root,"build/wasm-exp1/typed-values/correctness");await mkdir(outputRoot,{recursive:true});
const args=process.argv.slice(2);const moduleArg=args[0];
if(!moduleArg){const result={status:"not_run",reason:"A complete compiler-produced full-slice Wasm module must be supplied explicitly.",command:"bun run experiments/wasm-exp1/full/run-local.ts FULL_MODULE.wasm [PROGRAM.json] [--probe-only] [--mutated=MODULE.wasm]"};await writeFile(join(outputRoot,"not-run.json"),JSON.stringify(result,null,2)+"\n");console.log(JSON.stringify(result));process.exit(0);}
const modulePath=resolve(moduleArg);
const programPath=args[1]&&!args[1].startsWith("--")?resolve(args[1]):join(root,"experiments/wasm-exp1/compiler/build/projected/program.json");
const probeOnly=args.includes("--probe-only");const mutatedArg=args.find(x=>x.startsWith("--mutated="))?.slice(10);
const runDir=await mkdtemp(join(outputRoot,"run-"));
const bytes=await readFile(modulePath);const programBytes=await readFile(programPath);
const program=JSON.parse(programBytes.toString());const spec=JSON.parse(await readFile(join(root,"experiments/wasm-exp1/acceptance.json"),"utf8"));
const module=new WebAssembly.Module(bytes);
if(WebAssembly.Module.imports(module).length)throw new Error("Unapproved application host import");
configureRows(module,JSON.parse(await readFile(join(import.meta.dir,'compiler/build/row-codec.json'),'utf8')));
const mutatedModule=mutatedArg?new WebAssembly.Module(await readFile(resolve(mutatedArg))):undefined;
if(mutatedModule)configureRows(mutatedModule,JSON.parse(await readFile(join(import.meta.dir,'compiler/build/mutated-row-codec.json'),'utf8')));
const wat=`(module (memory (export "memory") 2 128)
(func (export "alloc") (param i32) (result i32) i32.const 1024)
(func (export "start") (param i32 i32 i32) (result i32) unreachable)
(func (export "resume") (param i32 i32 i32 i32) (result i32) unreachable)
(func (export "result_ptr") (result i32) i32.const 0)
(func (export "result_len") (result i32) i32.const 0))`;
const wabt=await wabtFactory();const parsed=wabt.parseWat("abi-trap.wat",wat);parsed.validate();const trapBytes=parsed.toBinary({canonicalize_lebs:true}).buffer;parsed.destroy();
await writeFile(join(runDir,"abi-trap.wat"),wat);await writeFile(join(runDir,"abi-trap.wasm"),trapBytes);
const trapModule=new WebAssembly.Module(trapBytes);
let actualTrap=false;try{(new WebAssembly.Instance(trapModule).exports.start as Function)(0,0,0);}catch(error){actualTrap=error instanceof WebAssembly.RuntimeError;}if(!actualTrap)throw new Error("Fault fixture did not produce a real Wasm trap");
const safeEvents:any[]=[];
const databasePath=join(runDir,"authority.sqlite");const db=new Database(databasePath,{strict:true});
const host=createSliceHost(module,program,spec,bunSqlite(db),{
  trapModule,
  emitEvent:event=>safeEvents.push(structuredClone(event)),
  reopen:async()=>{const reopened=new Database(databasePath,{readonly:true});try{return createStorage(program,bunSqlite(reopened)).snapshot();}finally{reopened.close();}},
});
await host.reset();
const readInput=spec.probe_cases[0].input;
const storageSmoke=await host.invoke("probe",readInput,"owner");
if(storageSmoke.output.kind!=="success"||storageSmoke.output.value!==spec.probe_cases[0].expect.value||storageSmoke.hostCalls!==1)throw new Error("Generated probe + real SQLite read smoke failed");
const isolationSmoke=await host.invoke("probe",readInput,"other");
if(isolationSmoke.output.kind!=="success"||isolationSmoke.output.value!==readInput.title)throw new Error("Generated probe + real SQLite outsider smoke failed");
const trapSmoke=await host.invoke("probe",readInput,"owner",{trap:true});
if(trapSmoke.output.kind!=="internal"||trapSmoke.hostCalls!==0)throw new Error("Actual Wasm trap was not contained");
const afterTrapSmoke=await host.invoke("probe",readInput,"owner");
if(afterTrapSmoke.output.kind!=="success"||afterTrapSmoke.output.value!==spec.probe_cases[0].expect.value)throw new Error("Valid request failed after actual trap");
const probe=await runProbeSuite(module,spec,{mutatedModule});
let full:any;
try{full=probeOnly?{status:"not_run",reason:"Explicit probe-only mode; full-slice capability not supplied.",results:spec.full_slice_cases.map((c:any)=>({id:c.id,status:"not_run"}))}:await runFullSuite(host,spec,program);}finally{db.close();}
const digest=(value:Buffer|Uint8Array)=>createHash("sha256").update(value).digest("hex");
const report={poolStats,schemaVersion:1,target:"Bun SQLite + generated core Wasm",command:process.argv.slice(1),artifact:modulePath,artifactHash:digest(bytes),projectionHash:digest(programBytes),checkedRevision:program.checkedRevision,source:spec.source,trapFixture:{scope:"handwritten ABI host-fault fixture, not compiler artifact",sha256:digest(trapBytes),actualRuntimeTrap:actualTrap},storageSmoke:{scope:"Read-only generated probe on real SQLite, not full-slice pass",owner:storageSmoke,outsider:isolationSmoke,trap:trapSmoke,afterTrap:afterTrapSmoke},probe,full,safeEvents,sentinelAbsent:!JSON.stringify(safeEvents).includes("WASM_EXP1_SECRET_SENTINEL")};
await writeFile(join(runDir,"results.json"),JSON.stringify(report,null,2)+"\n");await writeFile(join(runDir,"safe-events.jsonl"),safeEvents.map(e=>JSON.stringify(e)).join("\n")+"\n");
console.log(JSON.stringify({evidence:join(runDir,"results.json"),probe:{passed:probe.passed,failed:probe.failed,notRun:probe.notRun},full:probeOnly?full:{passed:full.passed,failed:full.failed,notRun:full.notRun},sentinelAbsent:report.sentinelAbsent},null,2));
if(probe.failed||full.failed||!report.sentinelAbsent)process.exitCode=1;
