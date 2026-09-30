// ABI/marshalling fixture measurement. This fixture is NOT compiler application code.
import {readFile,writeFile,mkdir,mkdtemp} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const root=resolve(import.meta.dir,'../../..');
const option=(name:string)=>Bun.argv.find(a=>a.startsWith(`--${name}=`))?.slice(name.length+3);
const wasmPath=resolve(option('wasm')??join(root,'experiments/wasm-exp1/rust-full/build/probe.wasm'));
const programPath=join(root,'experiments/wasm-exp1/compiler/build/projected/program.json');
const baseline=join(root,'build/wasm-exp1/baseline/generated/bun/target');
const hash=(bytes:Uint8Array|string)=>createHash('sha256').update(bytes).digest('hex');
if(option('child')){
  delete Bun.env.DATABASE_URL;delete Bun.env.JADPO_DEBUG_TARGET_STACKS;
  const started=performance.now();let instantiateMs:number|null=null;
  if(option('child')==='baseline')await import(join(baseline,'app.ts'));
  else {
    const [{Database},{createStorage},{bunSqlite}]=await Promise.all([import('bun:sqlite'),import('../storage/adapter.ts'),import('../storage/bun-sqlite.ts'),import('../full/host.ts')]);
    const [bytes,programBytes]=await Promise.all([readFile(wasmPath),readFile(programPath)]);
    const module=await WebAssembly.compile(bytes);
    const startInstance=performance.now();new WebAssembly.Instance(module);instantiateMs=performance.now()-startInstance;
    const db=new Database(Bun.env.SQLITE_PATH!,{strict:true});createStorage(JSON.parse(programBytes.toString()),bunSqlite(db)).setup();db.close();
  }
  console.log(JSON.stringify({readyMs:performance.now()-started,instantiateMs,rssBytes:process.memoryUsage().rss}));process.exit(0);
}
const outRoot=join(root,'build/wasm-exp1/boundary');await mkdir(outRoot,{recursive:true});
const out=await mkdtemp(join(outRoot,'run-'));
const stats=(values:number[])=>{const s=[...values].sort((a,b)=>a-b);const q=(p:number)=>s[Math.floor((s.length-1)*p)];return {samples:s.length,min:q(0),median:(s[Math.floor((s.length-1)/2)]+s[Math.floor(s.length/2)])/2,p95:q(.95),max:q(1),mean:s.reduce((a,b)=>a+b,0)/s.length};};
const report:any={schemaVersion:1,bunVersion:Bun.version,platform:process.platform,arch:process.arch,command:Bun.argv.slice(1),requiredRuntimeFlags:['--no-install','--env-file=/dev/null'],observedExecArgv:process.execArgv,startedUtc:new Date().toISOString(),scope:'Separate ABI helper, matched fresh-process startup, and raw artifact inventory; not application throughput or cloud performance',application:{wasm:wasmPath,sha256:hash(await readFile(wasmPath)),baselineAppSha256:hash(await readFile(join(baseline,'app.ts')))}};

// Both source sets are unbundled reproducible components. Gzip is each file alone.
async function inventory(files:string[]){const entries=[];for(const path of files){const bytes=await readFile(path);entries.push({path:resolve(path).replace(root+'/',''),bytes:bytes.length,gzipBytes:Bun.gzipSync(bytes,{level:9}).length,sha256:hash(bytes)});}return {entries,rawBytes:entries.reduce((n,f)=>n+f.bytes,0),sumIndividualGzipBytes:entries.reduce((n,f)=>n+f.gzipBytes,0)};}
report.inventory={
 baseline:await inventory([join(baseline,'app.ts'),join(baseline,'persistence.ts')]),
 wasmWithSqliteAdapters:await inventory([wasmPath,programPath,join(root,'experiments/wasm-exp1/host/driver.ts'),join(root,'experiments/wasm-exp1/full/host.ts'),join(root,'experiments/wasm-exp1/storage/adapter.ts'),join(root,'experiments/wasm-exp1/storage/bun-sqlite.ts')]),
 wasmWithCloudAdapters:await inventory([wasmPath,programPath,join(root,'experiments/wasm-exp1/host/driver.ts'),join(root,'experiments/wasm-exp1/full/host.ts'),join(root,'experiments/wasm-exp1/storage/adapter.ts'),join(root,'experiments/wasm-exp1/storage/cloudflare-sqlite.ts')]),
 limitations:['Unbundled components including trusted traced full host; no standalone HTTP entrypoint. Per-file gzip sums are not a deployment transfer size.','Wasm includes frozen checked projection consumed by generic SQL adapter.','No HTTP/auth routes exist in fixture; full test-suite/Worker report packaging is excluded.','Runtime host binaries and dev tools are excluded from application totals.','Both application sets use host builtins; no external application npm dependencies in this fixture.'],
};
if(!Bun.argv.includes('--inventory-only')){
 const {invoke,invokeSync,BUFFER_LIMIT}=await import('../host/driver.ts');
 const wabt=await (await import('../tooling/node_modules/wabt/index.js')).default();
 const prefix='{"kind":"pending","requestId":1,"operationId":1,"capability":"measurement.echo","args":';
 const encoder=new TextEncoder(),decoder=new TextDecoder();
 const prefixBytes=encoder.encode(prefix);
 const escape=(bytes:Uint8Array)=>[...bytes].map(b=>'\\'+b.toString(16).padStart(2,'0')).join('');
 const wat=`(module
  (memory (export "memory") 4 128)
  (global $heap (mut i32) (i32.const 4096))
  (global $ptr (mut i32) (i32.const 0)) (global $len (mut i32) (i32.const 0))
  (global $state (mut i32) (i32.const 0))
  (data (i32.const 64) "${escape(prefixBytes)}")
  (func (export "alloc") (param $n i32) (result i32) (local $p i32)
    global.get $heap local.tee $p local.get $n i32.add global.set $heap local.get $p)
  (func (export "start") (param i32) (param $input i32) (param $size i32) (result i32)
    i32.const 196608 i32.const 64 i32.const ${prefixBytes.length} memory.copy
    i32.const ${196608+prefixBytes.length} local.get $input local.get $size memory.copy
    i32.const ${196608+prefixBytes.length} local.get $size i32.add i32.const 125 i32.store8
    i32.const 196608 global.set $ptr local.get $size i32.const ${prefixBytes.length+1} i32.add global.set $len
    i32.const 1 global.set $state i32.const 2)
  (func (export "resume") (param $request i32) (param $operation i32) (param $input i32) (param $size i32) (result i32)
    global.get $state i32.const 1 i32.ne if unreachable end
    local.get $request i32.const 1 i32.ne if unreachable end
    local.get $operation i32.const 1 i32.ne if unreachable end
    i32.const 2 global.set $state local.get $input global.set $ptr local.get $size global.set $len i32.const 0)
  (func (export "result_ptr") (result i32) global.get $ptr)
  (func (export "result_len") (result i32) global.get $len))`;
 const parsed=wabt.parseWat('boundary-helper.wat',wat);parsed.validate();const helper=parsed.toBinary({canonicalize_lebs:true}).buffer;parsed.destroy();
 await writeFile(join(out,'boundary-helper.wat'),wat);await writeFile(join(out,'boundary-helper.wasm'),helper);
 const module=new WebAssembly.Module(helper);const results=[];
 // The actual fixture's optional note permits large validated input and stored-row
 // frames, so measure real compiler output too rather than claiming helper parity.
 const spec=JSON.parse(await readFile(join(root,'experiments/wasm-exp1/acceptance.json'),'utf8'));
 const program=JSON.parse(await readFile(programPath,'utf8'));
 const appModule=new WebAssembly.Module(await readFile(wasmPath));
 const entry=program.declarations.find((d:any)=>d.name===spec.scope.probe_entrypoint);
 const query=program.declarations.find((d:any)=>d.kind==='callable'&&d.body[0]?.value?.value?.op==='query');
 const row=spec.seeds[query.body[0].value.value.entity][0];
 const actualResults=[];
 for(const size of [256,4096,65536]){
  const input:any={id:row.id,title:'fallback',note:''};
  input.note='x'.repeat(size-encoder.encode(JSON.stringify(input)).length);
  const response:any={kind:'success',value:{...row,note:''}};
  response.value.note='y'.repeat(size-encoder.encode(JSON.stringify(response)).length);
  if(encoder.encode(JSON.stringify(input)).length!==size||encoder.encode(JSON.stringify(response)).length!==size)throw Error('Actual full-core frame size');
  const inspection:any=new WebAssembly.Instance(appModule).exports;
  const inspectionBytes=encoder.encode(JSON.stringify(input)),inspectionPointer=inspection.alloc(inspectionBytes.length);
  new Uint8Array(inspection.memory.buffer,inspectionPointer,inspectionBytes.length).set(inspectionBytes);
  const initialStatus=inspection.start(entry.semanticId,inspectionPointer,inspectionBytes.length);
  const initialResultFrameBytes=inspection.result_len();
  const iterations=size===256?50:size===4096?25:10;
  for(const mode of ['sync','async']){
   let hostCalls=0;let pendingBytes=0;
   const dispatch=(capability:string,args:any)=>{if(capability!=='storage.read'||args.entity!==query.body[0].value.value.entity||args.semanticOperationId!==query.semanticId)throw Error('Unexpected compiled capability');hostCalls++;pendingBytes=encoder.encode(JSON.stringify(args)).length;return response;};
   const call=()=>mode==='sync'?invokeSync(appModule,entry.semanticId,input,dispatch):invoke(appModule,entry.semanticId,input,async(c,a)=>dispatch(c,a));
   for(let n=0;n<50;n++)await call();const before=hostCalls;const repetitions=[];let errors=0;let firstError:any=null;
   for(let n=0;n<20;n++){const latencies=[];for(let i=0;i<iterations;i++){const start=performance.now();const value:any=mode==='sync'?call():await call();latencies.push(performance.now()-start);if(value.kind!=='success'||value.value!==row.title){errors++;firstError??=value;}}repetitions.push({run:n+1,...stats(latencies)});}
   actualResults.push({mode,inputFrameBytes:size,hostResponseFrameBytes:size,initialStatus,pendingFrameBytes:initialStatus===2?initialResultFrameBytes:null,hostArgumentBytes:pendingBytes,terminalFrameBytes:encoder.encode(JSON.stringify({kind:'success',value:row.title})).length,iterationsPerRun:iterations,runs:20,hostCalls:hostCalls-before,expectedHostCalls:20*iterations,errors,firstError,medianRunLatencyMs:stats(repetitions.map(r=>r.median)),repetitions});
  }
 }
 report.actualCompilerBoundary={scope:'Actual complete compiler-produced application module: closed input validation, asynchronous storage capability, stored-row validation and authored outcome recovery/projection. No database I/O. Optional input note and host row note size the complete frames exactly; these are not empty-crossing timings.',results:actualResults};
 await writeFile(join(out,'partial-results.json'),JSON.stringify(report,null,2)+'\n');
 for(const size of [256,4096,65536]){
  const base={data:''};const empty=encoder.encode(JSON.stringify(base)).length;
  const input={data:'x'.repeat(size-prefixBytes.length-1-empty)};
  const inputBytes=encoder.encode(JSON.stringify(input));const response={kind:'success',value:input};
  const responseBytes=encoder.encode(JSON.stringify(response));
  const pendingBytes=prefixBytes.length+inputBytes.length+1;
  if(pendingBytes!==size||Math.max(inputBytes.length,responseBytes.length,pendingBytes)>BUFFER_LIMIT)throw Error('Frame size');
  let hostCalls=0;
  const dispatch=(capability:string,args:any)=>{if(capability!=='measurement.echo')throw Error('Unexpected capability');hostCalls++;return {kind:'success',value:args};};
  const codec=()=>{const request=encoder.encode(JSON.stringify(input));const pending=new Uint8Array(prefixBytes.length+request.length+1);pending.set(prefixBytes);pending.set(request,prefixBytes.length);pending[pending.length-1]=125;const decoded=JSON.parse(decoder.decode(pending));return JSON.parse(decoder.decode(encoder.encode(JSON.stringify(dispatch(decoded.capability,decoded.args)))));};
  const sync=()=>invokeSync(module,1,input,dispatch);
  const asyncCall=()=>invoke(module,1,input,async(capability,args)=>dispatch(capability,args));
  const iterations=size===256?50:size===4096?25:10;
  for(const [name,call]of [['host_codec_control',codec],['wasm_sync_boundary',sync],['wasm_async_boundary',asyncCall]] as const){
   for(let i=0;i<50;i++)await call();
   const repetitions=[];let errors=0;const callsBefore=hostCalls;
   for(let n=0;n<20;n++){const latencies=[];for(let i=0;i<iterations;i++){const start=performance.now();const result:any=name==='wasm_async_boundary'?await call():call();latencies.push(performance.now()-start);if(result.kind!=='success'||result.value?.data!==input.data)errors++;}repetitions.push({run:n+1,...stats(latencies)});}
   if(errors||hostCalls-callsBefore!==20*iterations)throw Error('Boundary semantic/capability count failed');
   results.push({mode:name,largestEncodedFrameBytes:size,inputFrameBytes:inputBytes.length,pendingFrameBytes:pendingBytes,hostResponseFrameBytes:responseBytes.length,terminalFrameBytes:responseBytes.length,iterationsPerRun:iterations,runs:20,hostCalls:hostCalls-callsBefore,errors,medianRunLatencyMs:stats(repetitions.map(r=>r.median)),repetitions});
  }
 }
 report.boundary={helperSha256:hash(helper),helperBytes:helper.length,helperScope:'Handwritten ABI echo state machine; measures unchanged production experiment driver serialization/copies/fresh instance/pending/resume only. Does not parse or validate JSON inside Wasm or execute compiled application semantics.',memoryInitialBytes:4*65536,memoryMaxBytes:128*65536,dispatch:'No I/O or intentional delay; one exact capability per invocation',codecControl:'Equivalent input encoding, pending frame assembly/parse and response encoding/parse entirely in host; not generated Bun app or allocator-equivalent implementation.',results};
 if(!Bun.argv.includes('--skip-startup')){
  const samples:any[]=[];
  for(let n=0;n<20;n++)for(const target of n%2===0?['baseline','wasm']:['wasm','baseline']){
   const path=join(out,`startup-${target}-${n}.sqlite`);
   const env={...Bun.env,SQLITE_PATH:path};delete env.DATABASE_URL;delete env.JADPO_DEBUG_TARGET_STACKS;
   const start=performance.now();const child=Bun.spawn([process.execPath,'--no-install','--env-file=/dev/null','run',import.meta.path,`--child=${target}`,`--wasm=${wasmPath}`],{cwd:root,env,stdout:'pipe',stderr:'pipe'});
   const [stdout,stderr,exitCode]=await Promise.all([new Response(child.stdout).text(),new Response(child.stderr).text(),child.exited]);
   const wallMs=performance.now()-start;if(exitCode!==0)throw Error(`Startup ${target} failed: ${stderr}`);
   const result=JSON.parse(stdout);samples.push({target,run:n+1,parentWallMs:wallMs,...result});
  }
  report.startup={scope:'20 fresh Bun processes per target, alternating order. Baseline imports actual generated app (which initializes SQLite schema); Wasm compiles/instantiates actual complete module and initializes the checked SQL adapter/schema. Filesystem/OS caches are warm; not machine cold starts or Cloudflare isolates. Parent wall includes shared harness process loading, child ready excludes common top-level harness imports.',samples,summary:Object.fromEntries(['baseline','wasm'].map(target=>{const list=samples.filter(x=>x.target===target);return [target,{parentWallMs:stats(list.map(x=>x.parentWallMs)),childReadyMs:stats(list.map(x=>x.readyMs)),rssBytes:stats(list.map(x=>x.rssBytes))}];}))};
 }
}
for(const group of [report.inventory.baseline,report.inventory.wasmWithSqliteAdapters,report.inventory.wasmWithCloudAdapters])for(const entry of group.entries)if(hash(await readFile(join(root,entry.path)))!==entry.sha256)throw Error('Measured source changed during run: '+entry.path);
report.finishedUtc=new Date().toISOString();await writeFile(join(out,'results.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({evidence:join(out,'results.json'),actualCompilerBoundary:report.actualCompilerBoundary?.results.map((r:any)=>({mode:r.mode,frame:r.inputFrameBytes,medianMs:r.medianRunLatencyMs.median,errors:r.errors})),boundary:report.boundary?.results.map((r:any)=>({mode:r.mode,frame:r.largestEncodedFrameBytes,medianMs:r.medianRunLatencyMs.median,p95RunMedianMs:r.medianRunLatencyMs.p95})),startup:report.startup?.summary,inventory:{baseline:report.inventory.baseline.rawBytes,wasmSql:report.inventory.wasmWithSqliteAdapters.rawBytes,wasmCloud:report.inventory.wasmWithCloudAdapters.rawBytes}},null,2));

if(report.actualCompilerBoundary?.results.some((r:any)=>r.errors||r.hostCalls!==r.expectedHostCalls))process.exitCode=1;
