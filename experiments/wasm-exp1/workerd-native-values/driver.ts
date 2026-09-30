// Compiler-owned experimental host boundary. No application control flow here.
import {rowCodec,type Metadata} from './codec.ts';
const transports=new WeakMap<WebAssembly.Module,{flags:number;adaptive:boolean;direct:boolean;codec:ReturnType<typeof rowCodec>}>();
export function configureRows(module:WebAssembly.Module,meta:Metadata,flags=3,adaptive=false,direct=false){
 if(!Number.isInteger(flags)||flags<0||flags>15)throw Error('row-flags');
 transports.set(module,{flags,adaptive,direct,codec:rowCodec(meta)});
}
export const BUFFER_LIMIT = 65_536;
export const MEMORY_LIMIT = 128 * 65_536;
type Envelope = { kind: string; [name: string]: unknown };
export type Dispatch = (capability: string, args: unknown) => Promise<Envelope>;
type Core = {
  memory: WebAssembly.Memory;
  reset?: () => number;
  alloc(length: number): number;
  start(operation: number, pointer: number, length: number): number;
  resume(request: number, operation: number, pointer: number, length: number): number;
  result_ptr(): number;
  result_len(): number;
  set_row_transport?: (flags:number)=>number;
  result_format?: ()=>number;
  resume_ref?: (request:number,operation:number,pointer:number,length:number)=>number;
  resume_row_ref?: (request:number,operation:number,pointer:number,length:number)=>number;
  resume_row?: (request:number,operation:number,pointer:number,length:number)=>number;
};
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const internal = (): Envelope => ({ kind: 'internal', message: 'Unexpected internal failure.' });

export function boundedEncode(value: unknown): Uint8Array {
  const bytes = encoder.encode(JSON.stringify(value));
  if (bytes.length > BUFFER_LIMIT) throw new Error('boundary-size');
  return bytes;
}
function bounds(core: Core, pointer: number, length: number) {
  if (!Number.isInteger(pointer) || !Number.isInteger(length) || pointer < 0 || length < 0 ||
      length > BUFFER_LIMIT || core.memory.buffer.byteLength > MEMORY_LIMIT ||
      pointer > core.memory.buffer.byteLength - length) throw new Error('boundary-pointer');
}
function write(core: Core, value: unknown) {
  const json=JSON.stringify(value);
  if(typeof json!=='string'||json.length>BUFFER_LIMIT)throw Error('boundary-size');
  // UTF-8 needs at most three bytes per UTF-16 code unit. The guest accepts a
  // bounded prefix of its allocation; encodeInto reports the actual byte count.
  const capacity=Math.min(BUFFER_LIMIT,json.length*3);
  const pointer=core.alloc(capacity);
  if(!pointer)throw Error('boundary-allocation');
  bounds(core,pointer,capacity);
  const {read,written}=encoder.encodeInto(json,new Uint8Array(core.memory.buffer,pointer,capacity));
  if(read!==json.length)throw Error('boundary-size');
  return [pointer,written] as const;
}
type OwnedRows=Map<string,{schema:number,row:any}>;
function read(core: Core, transport:ReturnType<typeof transports.get>,operation:number,status:number,rows:OwnedRows): Envelope {
  const pointer = core.result_ptr(), length = core.result_len();
  bounds(core, pointer, length);
  // Borrow only during this synchronous decoder. It returns owned JS values;
  // no view survives an await, allocation, next guest call or memory growth.
  const bytes = new Uint8Array(core.memory.buffer, pointer, length);
  const format=core.result_format?.()??0;
  if(format===3){
   if(status!==2||!transport||(transport.flags&8)===0)throw Error('pending-format');
   return transport.codec.pending(bytes);
  }
  if(format===2){
   if(status!==0||!transport||(transport.flags&4)===0)throw Error('reference-format');
   return {kind:'success',value:transport.codec.reference(transport.codec.resultSchema(operation),bytes,rows)};
  }
  if(format!==0){
   if(format!==1||status!==0||!transport||(transport.flags&2)===0)throw Error('row-format');
   return {kind:'success',value:transport.codec.decode(transport.codec.resultSchema(operation),bytes)};
  }
  const result = JSON.parse(decoder.decode(bytes));
  if (!result || typeof result !== 'object' || Array.isArray(result) || typeof result.kind !== 'string')
    throw new Error('boundary-envelope');
  return result;
}
function resume(core:Core,transport:ReturnType<typeof transports.get>,pending:Envelope,result:Envelope,request:number,operation:number,rows:OwnedRows){
 const args=pending.args as any;
 const schema=transport?.codec.schema(args?.entity);
 let owned:any;
 if(transport&&(transport.flags&4)&&schema!==undefined&&pending.capability==='storage.read'&&result.kind==='success'&&result.value!==null&&Object.keys(result).length===2&&Object.hasOwn(result,'value')&&Object.values(result.value as object).some(v=>typeof v==='string'&&v.length>=1024)){
  try{
   const snapshot=transport.codec.snapshot(schema,result.value);
   if(Object.values(snapshot).some(v=>typeof v==='string'&&v.length>=1024)){owned=snapshot;rows.set(request+':'+operation,{schema,row:owned});result={kind:'success',value:owned};}
  }catch{/* No reference optimisation for unsupported host values. */}
 }
 if(transport&&(transport.flags&1)&&(!transport.adaptive||Object.values(result.value??{}).some(v=>typeof v==='string'&&/[\"\\\x00-\x1f]/.test(v)))&&core.resume_row&&pending.capability==='storage.read'&&result.kind==='success'&&result.value!==null&&schema!==undefined&&Object.values(result.value as object).some(v=>typeof v==='string'&&v.length>=1024)){
  if(Object.keys(result).length!==2||!Object.hasOwn(result,'value'))throw Error('row-envelope');
  let bytes:Uint8Array|undefined,p=0;
  try{
   if(transport.direct&&owned){
    // Private scalar snapshots have no accessors. A conservative capacity avoids
    // a sizing UTF-8 pass. The guest accepts the actual written prefix.
    const capacity=transport.codec.capacity(schema,owned);
    p=core.alloc(capacity);if(!p)throw Error('row-allocation');bounds(core,p,capacity);
    bytes=transport.codec.encode(schema,result.value,new Uint8Array(core.memory.buffer,p,capacity));
   }else {p=0;bytes=transport.codec.encode(schema,result.value);}
  }catch{/* JSON still enforces its size, shape and type checks. */}
  if(bytes){
   if(!p){p=core.alloc(bytes.length);if(!p)throw Error('row-allocation');bounds(core,p,bytes.length);new Uint8Array(core.memory.buffer,p,bytes.length).set(bytes);}
   // No allocation, await or guest call between obtaining this view and resume.
   return owned&&core.resume_row_ref?core.resume_row_ref(request,operation,p,bytes.length):core.resume_row(request,operation,p,bytes.length);
  }
 }
 const [p,n]=write(core,result);return owned&&core.resume_ref?core.resume_ref(request,operation,p,n):core.resume(request,operation,p,n);
}
// Idle instances only: an active request has an exclusive lease, including awaits.
const idle = new WeakMap<WebAssembly.Module, Core[]>();
export const poolStats = {created:0,reused:0,returned:0,discarded:0,active:0,peakActive:0,maxRetainedPages:0,maxIdle:0};
const MAX_IDLE = 8, MAX_RETAINED_PAGES = 32;
function acquire(module:WebAssembly.Module,imports:WebAssembly.Imports):Core {
  const pool=idle.get(module)??[]; idle.set(module,pool);
  let core:Core|undefined;
  // Imported mutable state is outside the reset contract: never pool such modules.
  if(Object.keys(imports).length===0 && WebAssembly.Module.imports(module).length===0){
    while(pool.length){const candidate=pool.pop()!;
      try{if(candidate.reset?.()===1){core=candidate;poolStats.reused++;break;}}catch{}
      poolStats.discarded++;
    }
  }
  if(!core){core=new WebAssembly.Instance(module,imports).exports as unknown as Core;poolStats.created++;}
  poolStats.active++;poolStats.peakActive=Math.max(poolStats.peakActive,poolStats.active);
  return core;
}
function release(module:WebAssembly.Module,core:Core|undefined,reusable:boolean,imports:WebAssembly.Imports){
  if(!core)return; poolStats.active--;
  const pool=idle.get(module)!;
  if(reusable&&typeof core.reset==='function'&&core.memory instanceof WebAssembly.Memory&&
     core.memory.buffer.byteLength<=MAX_RETAINED_PAGES*65536&&pool.length<MAX_IDLE&&
     Object.keys(imports).length===0&&WebAssembly.Module.imports(module).length===0){pool.push(core);poolStats.returned++;poolStats.maxRetainedPages=Math.max(poolStats.maxRetainedPages,core.memory.buffer.byteLength/65536);poolStats.maxIdle=Math.max(poolStats.maxIdle,pool.length);}
  else poolStats.discarded++;
}
export async function invoke(
  module: WebAssembly.Module, operation: number, input: unknown, dispatch: Dispatch,
  imports: WebAssembly.Imports = {},
): Promise<Envelope> {
  // One exclusive lease per invocation; reset is checked before an idle instance is reused.
  let leased:Core|undefined;let reusable=false;const rows:OwnedRows=new Map();
  try {
    const core = leased = acquire(module, imports);
    const transport=transports.get(module);
    if(transport&&core.set_row_transport?.(transport.flags)!==1)throw Error('row-negotiation');
    if (!(core.memory instanceof WebAssembly.Memory)) return internal();
    const [pointer, length] = write(core, input);
    let status = core.start(operation, pointer, length);
    let previousOperation = 0;
    let requestId: number | undefined;
    for (let steps = 0; steps < 64; steps++) {
      const output = read(core,transport,operation,status,rows);
      if (status !== 2) {
        const expected: Record<number, string> = {0:'success',1:'domain',3:'internal',4:'invalid'};
        if(expected[status]!==output.kind)return internal();
        reusable=status!==3; // Internal faults discard; successful/domain/invalid terminals may reset.
        return output;
      }
      if (output.kind !== 'pending' || !Number.isSafeInteger(output.requestId) ||
          !Number.isSafeInteger(output.operationId) || typeof output.capability !== 'string' ||
          (output.operationId as number) <= previousOperation ||
          (requestId !== undefined && output.requestId !== requestId)) return internal();
      requestId = output.requestId as number;
      previousOperation = output.operationId as number;
      let result: Envelope;
      try { result = await dispatch(output.capability, output.args); }
      catch { result = internal(); } // Never transport raw host errors.
      if (!result || !['success','domain','internal'].includes(result.kind)) result = internal();
      status = resume(core,transport,output,result,requestId,previousOperation,rows);
    }
    return internal();
  } catch {
    // A trap cannot enter an authored domain recovery arm. Discard the instance.
    return internal();
  } finally {release(module,leased,reusable,imports);}
}

// The authority invokes this inside a synchronous SQL transaction. The outer
// RPC/host call is awaited by the caller; no Promise crosses this transaction.
export function invokeSync(
  module: WebAssembly.Module, operation: number, input: unknown,
  dispatch: (capability: string, args: unknown) => Envelope,
  imports: WebAssembly.Imports = {},
): Envelope {
  let leased:Core|undefined;let reusable=false;const rows:OwnedRows=new Map();
  try {
    const core = leased = acquire(module, imports);
    const transport=transports.get(module);
    if(transport&&core.set_row_transport?.(transport.flags)!==1)throw Error('row-negotiation');
    if (!(core.memory instanceof WebAssembly.Memory)) return internal();
    const [pointer, length] = write(core, input);
    let status = core.start(operation, pointer, length);
    let previousOperation = 0;
    let requestId: number | undefined;
    for (let steps = 0; steps < 64; steps++) {
      const output = read(core,transport,operation,status,rows);
      if (status !== 2) {
        const expected: Record<number, string> = {0:'success',1:'domain',3:'internal',4:'invalid'};
        if(expected[status]!==output.kind)return internal();
        reusable=status!==3; // Internal faults discard; successful/domain/invalid terminals may reset.
        return output;
      }
      if (output.kind !== 'pending' || !Number.isSafeInteger(output.requestId) ||
          !Number.isSafeInteger(output.operationId) || typeof output.capability !== 'string' ||
          (output.operationId as number) <= previousOperation ||
          (requestId !== undefined && output.requestId !== requestId)) return internal();
      requestId = output.requestId as number;
      previousOperation = output.operationId as number;
      let result: Envelope;
      try { result = dispatch(output.capability, output.args); }
      catch { result = internal(); }
      if (!result || typeof (result as {then?: unknown}).then === 'function' ||
          !['success','domain','internal'].includes(result.kind)) result = internal();
      status = resume(core,transport,output,result,requestId,previousOperation,rows);
    }
    return internal();
  } catch { return internal(); } finally {release(module,leased,reusable,imports);}
}
