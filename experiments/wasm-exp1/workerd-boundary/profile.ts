import {rowCodec} from '../read-path/codec.ts';
import program from '../compiler/build/projected/program.json';
import spec from '../acceptance.json';
import metadata from '../read-path/compiler/build/row-codec.json';
const assert={equal(a:unknown,b:unknown){if(a!==b)throw Error('profile assertion '+String(a).slice(0,120)+' != '+String(b).slice(0,120));}};
function same(a:any,b:any):boolean {if(a===b)return true;if(!a||!b||typeof a!=='object'||typeof b!=='object')return false;const keys=Object.keys(a);return keys.length===Object.keys(b).length&&keys.every(k=>Object.hasOwn(b,k)&&same(a[k],b[k]));}
const enc=new TextEncoder(),dec=new TextDecoder('utf-8',{fatal:true});
export function profileStages(storage:any,module:WebAssembly.Module,options:{flags:number;size:number;kind:string;operation:string}) {
 if(![14,15].includes(options.flags)||![256,16384,49152].includes(options.size)||!['ascii','escaped'].includes(options.kind)||!['Item.read','Item.read_title'].includes(options.operation))throw Error('profile bounds');
 const principal={entity:'User',values:{id:spec.seeds.principals.owner.id}};
 const codec=rowCodec(metadata);
const results:any[]=[];
const {flags,size,kind,operation}=options;const escaped=kind==='escaped';
{
 const rows=structuredClone(spec.seeds.Item);rows[0].note=escaped?'é中😀"\\\n'.repeat(Math.floor(size/32)):'x'.repeat(size);
 storage.resetFixture({Item:rows});
 for(const name of [operation]){
  const core:any=new WebAssembly.Instance(module).exports;
  const op=program.declarations.find((d:any)=>d.name===name).semanticId;
  const ownedRows=new Map();
  const totals:Record<string,number>={},counts:Record<string,number>={};let record=false;
  const timed=(key:string,fn:()=>any)=>{const t=performance.now(),v=fn();if(record)totals[key]=(totals[key]??0)+performance.now()-t;return v;};
  const write=(key:string,value:any)=>{
   const json=timed(key+'_stringify',()=>JSON.stringify(value));
   const bytes=timed(key+'_utf8',()=>enc.encode(json));counts[key+'_bytes']=bytes.length;
   const p=timed(key+'_alloc_copy',()=>{const p=core.alloc(bytes.length);if(!p)throw Error('allocation');new Uint8Array(core.memory.buffer,p,bytes.length).set(bytes);return p;});return [p,bytes.length];
  };
  const read=(key:string)=>{
   const bytes=timed(key+'_copy',()=>new Uint8Array(core.memory.buffer,core.result_ptr(),core.result_len()).slice());counts[key+'_bytes']=bytes.length;
   if(core.result_format()===3)return timed('compact_host_decode',()=>codec.pending(bytes));
   if(core.result_format()===2)return timed('reference_host_decode',()=>({kind:'success',value:codec.reference(codec.schema('Item'),bytes,ownedRows)}));
   if(core.result_format()===1)return timed('binary_host_decode',()=>({kind:'success',value:codec.decode(codec.schema('Item'),bytes)}));
   const json=timed(key+'_utf8',()=>dec.decode(bytes));return timed(key+'_parse',()=>JSON.parse(json));
  };
  const run=()=>{
   assert.equal(timed('reset',()=>core.reset()),1);core.set_row_transport(flags);ownedRows.clear();
   const [p,n]=write('input',rows[0].id);assert.equal(timed('guest_start',()=>core.start(op,p,n)),2);
   const pending=read('pending');let result=timed('host_sql_and_validation',()=>storage.read(pending.args,principal));
   if(flags&4){const owned=timed('snapshot',()=>codec.snapshot(codec.schema('Item')!,result.value));ownedRows.set(pending.requestId+':'+pending.operationId,{schema:codec.schema('Item'),row:owned});result={kind:'success',value:owned};}
   let rp:number,rn:number;
   if(flags&1){const b=timed('host_binary_encode',()=>codec.encode(codec.schema('Item')!,result.value));counts.row_bytes=b.length;rp=timed('row_alloc_copy',()=>{const p=core.alloc(b.length);new Uint8Array(core.memory.buffer,p,b.length).set(b);return p;});rn=b.length;}
   else [rp,rn]=write('row',result);
   const resume=flags&1?(flags&4?core.resume_row_ref:core.resume_row):(flags&4?core.resume_ref:core.resume);
   assert.equal(timed('guest_resume_decode_validate_execute_encode',()=>resume(pending.requestId,pending.operationId,rp,rn)),0);
   const output=read('terminal');assert.equal(same(output,{kind:'success',value:name==='Item.read'?rows[0]:rows[0].title}),true);
   timed('http_stringify',()=>JSON.stringify(output));
  };
  for(let i=0;i<200;i++)run();record=true;for(let i=0;i<2000;i++)run();
  results.push({flags,name,size,escaped,counts,meanUs:Object.fromEntries(Object.entries(totals).map(([k,v])=>[k,v*1000/2000]))});
 }
}
return {scope:'Local workerd instrumented component means; observer overhead, not additive request p95. Every response checked.',results};
}
