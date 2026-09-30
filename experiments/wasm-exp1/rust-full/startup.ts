// Matched real-SQLite startup smoke for fresh Bun processes; fixture boundary only.
import {Database} from 'bun:sqlite';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
const [route,artifactArg,programArg]=process.argv.slice(2);
if(!['rust','bun'].includes(route)||!artifactArg||!programArg)throw new Error('startup.ts rust|bun ARTIFACT PROGRAM.json');
const started=performance.now(),artifact=resolve(artifactArg);
const spec=await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const program=await Bun.file(resolve(programArg)).json();
const dir=await mkdtemp(join(tmpdir(),'jadpo-exp1-startup-')),database=join(dir,'authority.sqlite');
delete Bun.env.DATABASE_URL;delete Bun.env.JADPO_DEBUG_TARGET_STACKS;Bun.env.SQLITE_PATH=database;
const phases:any={};let output:any,memory:any,db:Database|undefined,artifactSha256:string;
try {
 if(route==='rust'){
  const bytes=await Bun.file(artifact).arrayBuffer();let time=performance.now();
  const module=new WebAssembly.Module(bytes);phases.wasmCompileMs=performance.now()-time;
  const [{createSliceHost},{bunSqlite}]=await Promise.all([import('../full/host.ts'),import('../storage/bun-sqlite.ts')]);
  db=new Database(database,{strict:true});
  const host=createSliceHost(module,program,spec,bunSqlite(db),{reopen:async()=>({})});await host.reset();
  const originalInstance=WebAssembly.Instance;let instanceMemory:WebAssembly.Memory|undefined;
  (WebAssembly as any).Instance=new Proxy(originalInstance,{construct(target,args,newTarget){const began=performance.now();const instance=Reflect.construct(target,args,newTarget);phases.instantiateMs=performance.now()-began;instanceMemory=instance.exports.memory;return instance;}});
  time=performance.now();let result:any;try{result=await host.invoke('probe',spec.probe_cases[0].input,'owner');}finally{WebAssembly.Instance=originalInstance;}
  phases.invokeIncludingInstantiationMs=performance.now()-time;
  phases.finalLinearMemoryPages=instanceMemory!.buffer.byteLength/65536;
  output=result.output;phases.hostCalls=result.hostCalls;
  // Module memory declaration is observable without allocating another instance.
  memory=(WebAssembly.Module.exports(module).find(x=>x.name==='memory')as any)?.type??null;
  artifactSha256=new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
 }else{
  const time=performance.now();const app=await import(join(artifact,'bun/target/app.ts'));const {persistence}=await import(join(artifact,'bun/target/persistence.ts'));phases.generatedImportMs=performance.now()-time;
  db=new Database(database,{strict:true});
  for(const row of spec.seeds.Item)db.prepare('INSERT INTO "item" (id,owner_id,title,note) VALUES (?1,?2,?3,?4)').run(row.id,row.owner_id,row.title,row.note);
  const subject=spec.seeds.principals.owner.id;
  const context=app.captureOperation(null,Object.freeze({kind:'user',subject,authenticationStrength:'trusted_experiment',values:Object.freeze({user_id:subject})}));
  const input=app.experimentValidators.ProbeInput(spec.probe_cases[0].input,'input');const begin=performance.now();
  output={kind:'success',value:await app.experimentCallables.probe(input,context,persistence)};phases.invokeMs=performance.now()-begin;
  artifactSha256=new Bun.CryptoHasher('sha256').update(await Bun.file(join(artifact,'bun/target/app.ts')).arrayBuffer()).digest('hex');
 }
 const pass=output.kind==='success'&&output.value==='alpha';
 console.log(JSON.stringify({route,pass,output,checkedRevision:program.checkedRevision,artifactSha256,scriptTotalMs:performance.now()-started,phases,residentSetBytes:process.memoryUsage().rss,runtimeResourceUsage:process.resourceUsage(),memoryDeclaration:memory??null,scope:'Fresh process, real disposable SQLite setup/seed, checked fixture owner, one protected semantic read; no network/artificial delay.'}));
 if(!pass)process.exitCode=1;
}finally{db?.close();await rm(dir,{recursive:true,force:true});}
