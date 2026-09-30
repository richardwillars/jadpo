import { invoke } from '../host/driver.ts';
const base = new URL('./build/',import.meta.url);
const module = await WebAssembly.compile(await Bun.file(new URL('probe.wasm',base)).arrayBuffer());
const acceptance = await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const manifest = await Bun.file(new URL('manifest.json',base)).json();
const entry = manifest.entry.semanticId;
const input = acceptance.probe_cases[0].input, row = acceptance.probe_cases[0].host.read_row;
const records: any[] = [];
function record(id:string,pass:boolean,actual:unknown) { records.push({id,pass,actual}); }
for (const [id,host] of [
  ['missing-value',{kind:'success'}],
  ['extra-envelope-field',{kind:'success',value:row,extra:true}],
  ['forged-domain',{kind:'domain',failure:'ItemMissing'}],
  ['wrong-host-row-kind',{kind:'success',value:[]}],
  ['extra-host-row-field',{kind:'success',value:{...row,extra:true}}],
  ['missing-required-nullable',{kind:'success',value:{id:row.id,owner_id:row.owner_id,title:row.title}}],
  ['invalid-stored-refinement',{kind:'success',value:{...row,title:'x'}}],
  ['invalid-stored-uuid',{kind:'success',value:{...row,id:'not-a-uuid'}}],
] as const) {
  const actual = await invoke(module,entry,input,async()=>host);
  record(id,actual.kind==='internal'&&actual.operation===26,actual);
}
for (const [title,kind] of [['😀😀😀','success'],['😀😀','invalid'],['e\u0301','invalid'],['e\u0301x','success']] as const) {
  let hostCalls=0;
  const actual = await invoke(module,entry,{...input,title},async()=>{hostCalls++;return {kind:'success',value:null};});
  record('unicode-scalar-'+title,actual.kind===kind&&(kind==='invalid'?hostCalls===0:actual.value===title),{actual,hostCalls});
}
const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8',{fatal:true});
async function core() {return (await WebAssembly.instantiate(module)).exports as any;}
function output(c:any) {return JSON.parse(decoder.decode(new Uint8Array(c.memory.buffer,c.result_ptr(),c.result_len())));}
function write(c:any,value:unknown) {
  const bytes=encoder.encode(JSON.stringify(value)),pointer=c.alloc(bytes.length);
  if(!pointer)throw new Error('allocation');
  new Uint8Array(c.memory.buffer,pointer,bytes.length).set(bytes);
  return [pointer,bytes.length] as const;
}
for(const [pointer,length] of [[0,1],[-1,1],[1,-1],[1,65537],[2147483647,3]]) {
  const c=await core(),status=c.start(entry,pointer,length);
  record('unallocated-buffer',status===4&&output(c).kind==='invalid',{pointer,length,status,output:output(c)});
}
{
  const c=await core(),status=c.start(entry,...write(c,input)),pending=output(c);
  const reply=write(c,{kind:'success',value:row});
  const wrong=c.resume(pending.requestId+1,pending.operationId,...reply);
  const retried=c.resume(pending.requestId,pending.operationId,...reply);
  record('wrong-resume-permanently-terminal',status===2&&wrong===3&&retried===3&&output(c).kind==='internal',{wrong,retried});
}
{
  const c=await core();c.start(entry,...write(c,input));
  const pending=output(c),status=c.resume(pending.requestId,pending.operationId,1,4);
  record('host-unallocated-buffer',status===3&&output(c).kind==='internal',{status,actual:output(c)});
}
{
  const c=await core(),first=c.start(entry,...write(c,input));
  const second=c.start(entry,...write(c,input));
  record('second-start-terminal',first===2&&second===3&&output(c).kind==='internal',{first,second});
}
{
  const c=await core();const initial=c.memory.buffer.byteLength/65536;
  c.memory.grow(128-initial);let trapped=false;
  try {c.memory.grow(1);} catch(error) {trapped=error instanceof RangeError;}
  record('bounded-memory',trapped&&c.memory.buffer.byteLength===8388608,{initialPages:initial,maximumPages:c.memory.buffer.byteLength/65536});
  record('bounded-allocation',c.alloc(65537)===0&&c.alloc(-1)===0,{});
}
record('no-host-imports',WebAssembly.Module.imports(module).length===0,WebAssembly.Module.imports(module));
record('only-public-abi-exports',WebAssembly.Module.exports(module).every(x=>['memory','alloc','result_ptr','result_len','__data_end','__heap_base','start','resume'].includes(x.name)),WebAssembly.Module.exports(module));
await Bun.write(new URL('edge-results.json',base),JSON.stringify(records,null,2)+'\n');
console.log(JSON.stringify({passed:records.filter(x=>x.pass).length,failed:records.filter(x=>!x.pass)}));
if(records.some(x=>!x.pass))process.exitCode=1;
