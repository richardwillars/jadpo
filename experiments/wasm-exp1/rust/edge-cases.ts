import { invoke } from '../host/driver.ts';
const module = await WebAssembly.compile(await Bun.file(new URL('build/probe.wasm',import.meta.url)).arrayBuffer());
const acceptance = await Bun.file(new URL('../acceptance.json',import.meta.url)).json();
const manifest = await Bun.file(new URL('build/manifest.json',import.meta.url)).json();
const input = acceptance.probe_cases[0].input;
const row = acceptance.seeds.Item[0];
const entry = manifest.entry.semanticId;
const results: any[] = [];
const encoder = new TextEncoder(), decoder = new TextDecoder();
async function core() { return (await WebAssembly.instantiate(module,{})).exports as any; }
function put(c:any,value:any,raw=false) {
  const bytes=encoder.encode(raw?value:JSON.stringify(value)); const pointer=c.alloc(bytes.length);
  if(!pointer) throw new Error('allocation');
  new Uint8Array(c.memory.buffer,pointer,bytes.length).set(bytes);
  return [pointer,bytes.length] as const;
}
function output(c:any) {return JSON.parse(decoder.decode(new Uint8Array(c.memory.buffer,c.result_ptr(),c.result_len())));}
function record(id:string,pass:boolean,details:any) {results.push({id,pass,...details});}
{
  const c=await core(), [p,n]=put(c,'{"id":',true), status=c.start(entry,p,n);
  record('P12',status===4&&output(c).kind==='invalid',{status,output:output(c)});
}
for(const mutation of ['request','operation','duplicate','after-internal']) {
  const c=await core(); const [p,n]=put(c,input);
  const status=c.start(entry,p,n), pending=output(c);
  let [q,m]=put(c,mutation==='after-internal'?{kind:'internal'}:{kind:'success',value:row});
  if(mutation==='duplicate'||mutation==='after-internal') c.resume(pending.requestId,pending.operationId,q,m);
  const result=c.resume(pending.requestId+(mutation==='request'?1:0),pending.operationId+(mutation==='operation'?1:0),q,m);
  record('P14-'+mutation,status===2&&result===3&&output(c).kind==='internal',{status:result,output:output(c)});
}
for(const value of [
  {kind:'success'}, {kind:'success',value:row,extra:'forbidden'},
  {kind:'domain',failure:'ItemMissing'}, {kind:'success',value:{...row,title:'x'}},
  {kind:'success',value:{...row,extra:'forbidden'}}, {kind:'success',value:[row]},
]) {
  const result=await invoke(module,entry,input,async()=>value);
  record('malformed-host',result.kind==='internal',{host:value,result});
}
{
  const c=await core(); const [p,n]=put(c,input); c.start(entry,p,n); const pending=output(c);
  const [q,m]=put(c,'{"kind":',true), status=c.resume(pending.requestId,pending.operationId,q,m);
  record('malformed-host-json',status===3&&output(c).kind==='internal',{status,output:output(c)});
}
for(const [pointer,length] of [[-1,1],[2147483647,65536],[0,1],[1,-1],[1,65537]]) {
  const c=await core(), status=c.start(entry,pointer,length);
  record('invalid-pointer',status===4,{pointer,length,status});
}
{
  const c=await core(); let capped=false;
  const before=c.memory.buffer.byteLength/65536;
  c.memory.grow(128-before);
  try {c.memory.grow(1);} catch {capped=true;}
  record('memory-cap',capped&&c.memory.buffer.byteLength===8*1024*1024,{initialPages:before,finalPages:c.memory.buffer.byteLength/65536});
  record('oversized-allocation',c.alloc(65537)===0&&c.alloc(-1)===0,{});
}
{
  const result=await invoke(module,entry,input,async()=>({kind:'success',value:{...row,note:'x'.repeat(65536)}}));
  record('oversized-host-envelope',result.kind==='internal',{result});
}
{
  const result=await invoke(module,entry,{...input,title:'😀😀😀'},async()=>({kind:'success',value:null}));
  record('unicode-scalars',result.kind==='success'&&result.value==='😀😀😀',{result});
}
console.log(JSON.stringify(results,null,2));
await Bun.write(new URL('build/edge-results.json',import.meta.url),JSON.stringify(results,null,2)+'\n');
if(results.some(x=>!x.pass))process.exitCode=1;
