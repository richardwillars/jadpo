// Trusted experiment boundary around compiler-generated callables. No application logic.
import { Database } from "bun:sqlite";
import { AsyncLocalStorage } from "node:async_hooks";
import { resolve, join, basename } from "node:path";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";

const root = resolve(import.meta.dir, "../../..");
const out = join(root, "build/wasm-exp1/baseline");
const compiler = join(root, "experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection");
const frozen = join(root, "experiments/wasm-exp1/fixture");
const acceptance = JSON.parse(await readFile(join(root, "experiments/wasm-exp1/acceptance.json"), "utf8"));
const source = await readFile(join(frozen, "app.jadpo"), "utf8");
const hash = (value: string | Buffer) => createHash("sha256").update(value).digest("hex");
assert.equal(hash(source), acceptance.source.sha256, "frozen source changed");
await mkdir(out, {recursive:true});
delete Bun.env.DATABASE_URL;
delete Bun.env.JADPO_DEBUG_TARGET_STACKS;
const context = new AsyncLocalStorage<any>();
const originalPrepare = Database.prototype.prepare;
const originalExec = Database.prototype.exec;
Database.prototype.prepare = function(sql: string, ...rest: any[]) {
  const statement = originalPrepare.call(this, sql, ...rest);
  return new Proxy(statement, {get(target, key) {
    const member = Reflect.get(target, key, target);
    if (typeof member !== "function") return member;
    return (...args: any[]) => {
      if (["all", "get", "run"].includes(String(key))) context.getStore()?.sql.push({sql, parameters: args});
      return member.apply(target, args);
    };
  }});
} as any;
Database.prototype.exec = function(sql: string, ...args: any[]) {
  context.getStore()?.sql.push({sql, parameters:args});
  return originalExec.call(this, sql, ...args);
} as any;

async function build(project: string, destination: string) {
  const run = Bun.spawnSync([compiler, project, destination, "--bun"], {cwd:root,env:{...Bun.env,DATABASE_URL:undefined}});
  await writeFile(join(out, `${basename(destination)}-build.log`), Buffer.concat([run.stdout,run.stderr]));
  assert.equal(run.exitCode,0,"checked projection failed");
  return JSON.parse(await readFile(join(destination,"program.json"),"utf8"));
}
async function load(destination: string) {
  const dbPath = join(destination,"case.sqlite");
  Bun.env.SQLITE_PATH=dbPath;
  const app = await import(join(destination,"bun/target/app.ts"));
  const {persistence} = await import(join(destination,"bun/target/persistence.ts"));
  assert.equal(typeof app.DomainFailure,"function","projection must export DomainFailure");
  const db = new Database(dbPath,{strict:true});
  return {app,persistence,db,dbPath,destination};
}
const program = await build(frozen,join(out,"generated"));
const base = await load(join(out,"generated"));
const seeds = acceptance.seeds.Item;
const reset = (env=base) => {env.db.exec('DELETE FROM "item"');for(const row of seeds) env.db.prepare('INSERT INTO "item" (id,owner_id,title,note) VALUES (?1,?2,?3,?4)').run(row.id,row.owner_id,row.title,row.note);};
const snapshot = (env=base) => env.db.prepare('SELECT id,owner_id,title,note FROM "item" ORDER BY id').all();
let requestCounter=0;
const completed: string[]=[];
const operationalLogs:any[]=[];
const originalConsoleError=console.error;
console.error=(message:any)=>{ const trace=context.getStore(); if(!trace)return originalConsoleError(message); const event=JSON.parse(String(message));trace.logs.push(event);operationalLogs.push(event); };
function instrument(client:any, trace:any, control:any={}, depth=0):any {
  return new Proxy(client,{get(target,key) {
    const fn=target[key];if(typeof fn!=="function")return fn;
    if(key==="withPolicy" || key==="withOperationTime") return (...args:any[])=>instrument(fn.apply(target,args),trace,control,depth);
    if(key==="transaction")return (work:any)=>fn.call(target,(child:any)=>work(instrument(child,trace,control,depth+1)));
    if(!String(key).startsWith("query_required_")&&!String(key).startsWith("update_required_"))return fn.bind(target);
    return async(...args:any[])=>{
      const call={capability:String(key),args:structuredClone(args),depth,phase:"started",started:performance.now(),finished:null as number|null};trace.host.push(call);
      if(control.delay_ms)await Bun.sleep(control.delay_ms);
      if(control.fault)throw new Error(control.fault);
      const value=await fn.apply(target,args);
      call.phase="completed";call.finished=performance.now();
      return Object.hasOwn(control,"result")?structuredClone(control.result):value;
    };
  }});
}
async function invoke(operation:string,input:any,principal="owner",control:any={},env=base,raw?:string) {
  const trace:any={requestId:`baseline-${++requestCounter}`,operation,host:[],sql:[],logs:[]};
  const {app}=env;let args:any[];
  try {
    const received=raw===undefined?structuredClone(input):JSON.parse(raw);
    const id=(value:any)=>app.experimentValidators.User({id:value},"input").id;
    const title=(value:any)=>app.experimentValidators.ItemTitle(value,"input.title");
    if(operation==="probe")args=[app.experimentValidators.ProbeInput(received,"input")];
    else if(operation==="Item.read")args=[id(received[0])];
    else if(operation==="Item.change")args=[id(received[0]),app.experimentValidators.ItemPatch(received[1],"input.patch")];
    else if(operation==="Item.rename")args=[id(received[0]),title(received[1])];
    else if(operation==="update_pair")args=[id(received[0]),title(received[1]),id(received[2]),title(received[3])];
    else throw new Error("Unsupported harness operation");
  }catch(error){
    if(!(error instanceof app.ValidationError)&&!(error instanceof SyntaxError))throw error;
    return {...trace,output:{kind:"invalid"},host_calls:0};
  }
  const subject=acceptance.seeds.principals[principal].id;
  const trusted=Object.freeze({kind:"user",subject,authenticationStrength:"trusted_experiment",values:Object.freeze({user_id:subject,...(control.private_context===undefined?{}:{private_test_context:control.private_context})})});
  const captured=app.captureOperation(null,trusted);
  let output:any;
  // Logs are collected per AsyncLocalStorage request; never include raw errors.
  try {
    output=await context.run(trace,async()=>{
      try{return {kind:"success",value:await app.experimentCallables[operation](...args,captured,instrument(env.persistence,trace,control))};}
      catch(error){
        if(error instanceof app.DomainFailure)return {kind:"domain",failure:error.failureName};
        const semanticId=acceptance.checked_semantic_facts.operation_name_spans.find((p:any)=>p.name===operation)?.id;
        app.reportRuntimeFault("RUNTIME_UNHANDLED_FAULT",String(semanticId),program.checkedRevision,trace.requestId,error);
        return {kind:"internal",operation:semanticId,message:"Unexpected internal failure."};
      }
    });
  } finally {completed.push(trace.requestId);}
  return {...trace,output,host_calls:trace.host.length};
}
function expected(actual:any,wanted:any){for(const [key,value]of Object.entries(wanted)){if(key==="host_calls")assert.equal(actual.host_calls,value);else assert.deepEqual(actual.output[key],value);}}
const results:any[]=[];
async function record(id:string,work:()=>Promise<any>){try{const evidence=await work();results.push({id,status:"pass",...evidence});}catch(error){results.push({id,status:"fail",reason:String(error)});}}
const probes=acceptance.probe_cases;
for(const p of probes.filter((p:any)=>Number(p.id.slice(1))<=12))await record(p.id,async()=>{
  reset();const before=snapshot();const control:any={delay_ms:p.host?.delay_ms,fault:p.host?.fault};
  if(p.id==="P06")control.result=p.host.read_row;
  const actual=await invoke("probe",p.input,p.principal,control,base,p.raw_input);expected(actual,p.expect);
  assert.deepEqual(snapshot(),before);if(p.id==="P05")assert(!JSON.stringify(actual).includes(p.host.fault));
  if(p.expect.host_calls===undefined)assert.equal(actual.host_calls,1);
  return {actual,storage_before:before,storage_after:snapshot()};
});
async function interleave(delays=[19,1]) {
  const cases=probes.find((p:any)=>p.id==="P13").interleaved;const start=completed.length;
  const actual=await Promise.all(cases.map((p:any,i:number)=>invoke("probe",p.input,p.principal,{delay_ms:delays[i]})));
  actual.forEach((a:any,i:number)=>{expected(a,cases[i].expect);assert.equal(a.host_calls,1);assert.equal(completed.filter(x=>x===a.requestId).length,1);});
  assert.notEqual(actual[0].requestId,actual[1].requestId);
  if(delays[0]>delays[1])assert.equal(completed[start],actual[1].requestId);
  return actual;
}
await record("P13",async()=>{reset();return {actual:await interleave()};});
results.push({id:"P14",status:"not_applicable",reason:"The generated Bun baseline uses Promises and has no Wasm resume(requestId,operationId) ABI. No resume safety claim."});
await record("P15",async()=>{
  reset();const p=probes.find((p:any)=>p.id==="P15");const before=await invoke("probe",p.input);expected(before,p.expect_before);
  const dir=join(out,"mutated-source");await mkdir(dir,{recursive:true});await writeFile(join(dir,"app.jadpo"),source.replace(p.source_mutation.find,p.source_mutation.replace));
  const changed=await build(dir,join(out,"mutated"));assert.notEqual(changed.checkedRevision,program.checkedRevision);
  const env=await load(join(out,"mutated"));reset(env);const after=await invoke("probe",p.input,"owner",{},env);expected(after,p.expect_after);
  assert.equal(hash(await readFile(join(frozen,"app.jadpo"))),acceptance.source.sha256);env.db.close();
  return {before,after,mutated_revision:changed.checkedRevision,mutated_source_hash:hash(await readFile(join(dir,"app.jadpo")))};
});

// Real SQLite storage/policy/transaction cases. Instrumentation observes calls only.
const full=acceptance.full_slice_cases;
await record("A01",async()=>{reset();const p=full[0];const actual=await invoke(p.operation,p.args,p.principal);expected(actual,p.expect);return {actual};});
await record("A02",async()=>{const attempts=[];for(const args of [["bad",{title:"valid"}],[seeds[0].id,{title:"ab"}],[seeds[0].id,{title:"abcdefghijklm"}],[seeds[0].id,{owner_id:seeds[2].owner_id}]]){reset();const before=snapshot();const a=await invoke("Item.change",args);expected(a,{kind:"invalid",host_calls:0});assert.deepEqual(snapshot(),before);attempts.push(a);}assert(results.filter(r=>["P07","P08","P09","P10","P11","P12"].includes(r.id)).every(r=>r.status==="pass"));return {actual:attempts};});
await record("A03",async()=>{reset();const actual=[];for(const p of full.find((p:any)=>p.id==="A03").sequence){const a=await invoke("Item.change",p.args);expected(a,{kind:"success",value:p.expect_row});assert.deepEqual(snapshot()[0],p.expect_row);actual.push(a);}const a=await invoke("Item.change",[seeds[0].id,{note:""}]);assert.equal(a.output.value.note,"");actual.push(a);return {actual,storage_after:snapshot()};});
for(const id of ["A04","A05","A06","A10"])await record(id,async()=>{
 const actual=[];for(const p of full.find((p:any)=>p.id===id).cases){reset();const before=snapshot();const op=p.operation??full.find((p:any)=>p.id===id).operation;const a=await invoke(op,p.args,p.principal??"owner");expected(a,p.expect_failure?{kind:"domain",failure:p.expect_failure}:{kind:p.expect_kind});if(a.output.kind!=="success")assert.deepEqual(snapshot(),before);if(id==="A10"&&a.output.kind==="success"){assert.equal(snapshot()[0].title,"first-new");assert.equal(snapshot()[1].title,"second-new");assert.equal(a.host_calls,2);}actual.push({...a,storage_before:before,storage_after:snapshot()});}
 if(id==="A05"){const a=await invoke("Item.read",[acceptance.seeds.missing_item_id]);expected(a,{kind:"domain",failure:"ItemMissing"});actual.push(a);}return {actual};
});
await record("A07",async()=>{const actual=[];for(const principal of ["owner","other"])for(const op of ["Item.read","Item.change","Item.rename"]){reset();const args=op==="Item.read"?[seeds[0].id]:[seeds[0].id,op==="Item.change"?{title:"renamed"}:"renamed"];const a=await invoke(op,args,principal);const scoped=a.sql.filter((s:any)=>s.sql.includes('"owner_id" = ?'));assert(scoped.length>0);for(const s of scoped){assert(s.sql.includes('"id" = ?'));assert(s.parameters.includes(seeds[0].id));assert(s.parameters.includes(acceptance.seeds.principals[principal].id));assert(!s.sql.includes(seeds[0].id));}expected(a,principal==="owner"?{kind:"success"}:{kind:"domain",failure:"ItemMissing"});actual.push(a);}return {actual};});
await record("A08",async()=>{reset();const actual=[];for(const replacement of [{...seeds[0],id:"not-a-uuid"},{...seeds[0],title:"x"},{...seeds[0],note:42},{kind:"wrong"},{id:seeds[0].id}]){const a=await invoke("Item.read",[seeds[0].id],"owner",{result:replacement});expected(a,{kind:"internal"});actual.push(a);}return {actual,control:"trusted persistence return corruption; no fixture-specific application replacement"};});
await record("A09",async()=>{assert(results.filter(r=>["P04","P05"].includes(r.id)).every(r=>r.status==="pass"));return {evidence_cases:["P04","P05"]};});
await record("A11",async()=>{reset();const p=full.find((p:any)=>p.id==="A11");const a=await invoke(p.operation,p.args);expected(a,{kind:"success"});assert(a.host.every((h:any)=>h.depth>=2));assert.equal(a.sql.filter((s:any)=>s.sql==="BEGIN IMMEDIATE").length,1);assert.equal(a.sql.filter((s:any)=>s.sql==="COMMIT").length,1);const reads=await Promise.all([invoke("Item.read",[seeds[0].id]),invoke("Item.read",[seeds[1].id])]);assert.deepEqual(reads.map(r=>r.output.value.title),["first-new","second-new"]);return {actual:a,authoritative_reads:reads};});
await record("A13",async()=>{reset();const actual=[];const p=full.find((p:any)=>p.id==="A13");for(let i=0;i<p.pair_count;i++)actual.push(await interleave(p.delays_ms[i%p.delays_ms.length]));const fault=await invoke("probe",probes[0].input,"owner",{fault:"WASM_EXP1_SECRET_SENTINEL"});const valid=await invoke("probe",probes[0].input);expected(fault,{kind:"internal"});expected(valid,{kind:"success",value:"alpha"});return {actual,after_fault:[fault,valid]};});
await record("A14",async()=>{reset();const a=await invoke("probe",probes[0].input,"owner",{fault:"WASM_EXP1_SECRET_SENTINEL",private_context:"WASM_EXP1_SECRET_SENTINEL"});assert(!JSON.stringify({output:a.output,logs:a.logs}).includes("WASM_EXP1_SECRET_SENTINEL"));expected(a,{kind:"internal"});return {actual:a,scope:"safe adapter envelope/logs; no HTTP route exists in frozen source, therefore no response header claim"};});
await record("A15",async()=>{reset();const actual=[];for(const op of ["Item.read","Item.rename","update_pair"]){const args=op==="Item.read"?[seeds[0].id]:op==="Item.rename"?[seeds[0].id,"valid"]:[seeds[0].id,"first-new",seeds[1].id,"second-new"];const a=await invoke(op,args,"owner",{fault:"private"});expected(a,{kind:"internal"});const mapping=acceptance.checked_semantic_facts.operation_name_spans.find((x:any)=>x.name===op);assert.equal(a.logs[0].semanticOperationId,String(mapping.id));assert.equal(source.slice(mapping.start,mapping.end),op.includes(".")?op.split(".")[1]:op);actual.push({...a,source_mapping:mapping});}return {actual,source_hash:hash(source),telemetry_scope:"trusted boundary emits top-level checked semantic operation identity; not innermost-call stack mapping"};});
await record("A16",async()=>{const actual=[];for(const p of full.find((p:any)=>p.id==="A10").cases.filter((p:any)=>p.expect_failure==="ItemMissing"||p.expect_kind==="success")){reset();const before=snapshot();const a=await invoke("update_pair",p.args);const reopened=new Database(base.dbPath,{readonly:true});const rows=reopened.prepare('SELECT id,owner_id,title,note FROM item ORDER BY id').all();reopened.close();if(p.expect_failure)assert.deepEqual(rows,before);else {assert.equal(rows[0].title,"first-new");assert.equal(rows[1].title,"second-new");}actual.push({...a,storage_before:before,storage_reopened:rows});}return {actual};});
await record("A12",async()=>{reset();const p=full.find((p:any)=>p.id==="A11");const a=await invoke(p.operation,p.args,"owner",{delay_ms:7});expected(a,{kind:"success"});assert.equal(a.host.length,2);assert(a.host[0].finished!==null && a.host[1].started>=a.host[0].finished);assert.equal(completed.filter(id=>id===a.requestId).length,1);return {actual:a,wasm_pending_ids:"not_applicable",evidence_cases:["P01","P04","P14"]};});
results.push({id:"A17",status:"not_run",reason:"Backend capability rejection belongs to route/host lowering; normal Bun supports constructs deliberately outside the Wasm slice."});
await record("A18",async()=>{const rebuilt=join(out,"rebuilt");await build(frozen,rebuilt);const hashes:any={};for(const file of ["program.json","bun/target/app.ts","bun/target/persistence.ts","bun/sql/sqlite/schema.sql"]){const a=hash(await readFile(join(base.destination,file)));const b=hash(await readFile(join(rebuilt,file)));assert.equal(a,b);hashes[file]=a;}assert.equal(results.find(r=>r.id==="P15").status,"pass");return {hashes,source_hash:hash(source),compiler_binary_hash:hash(await readFile(compiler)),bun_binary_hash:hash(await readFile(process.execPath)),command:[compiler,frozen,rebuilt,"--bun"],scope:"identical checked projection/generated sources; no standalone native binary reproduction claim"};});
const artifact=await readFile(join(base.destination,"bun/target/app.ts"));
const report={target:"Bun baseline",command:"bun --no-install --env-file=/dev/null run experiments/wasm-exp1/baseline/run.ts",source_hash:hash(source),artifact_hash:hash(artifact),projection_hash:hash(await readFile(join(base.destination,"program.json"))),checked_revision:program.checkedRevision,bun_version:Bun.version,results,counts:results.reduce((a:any,r:any)=>(a[r.status]=(a[r.status]??0)+1,a),{})};
await writeFile(join(out,"results.json"),JSON.stringify(report,null,2)+"\n");
console.log(JSON.stringify({counts:report.counts,failed:results.filter(r=>r.status==="fail"),evidence:join(out,"results.json")},null,2));
base.db.close();console.error=originalConsoleError;Database.prototype.prepare=originalPrepare;Database.prototype.exec=originalExec;
if(results.some(r=>r.status==="fail"))process.exitCode=1;
