// Compiler-owned experimental host boundary. No application control flow here.
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
  const bytes = boundedEncode(value);
  const pointer = core.alloc(bytes.length);
  if (!pointer) throw new Error('boundary-allocation');
  bounds(core, pointer, bytes.length);
  new Uint8Array(core.memory.buffer, pointer, bytes.length).set(bytes);
  return [pointer, bytes.length] as const;
}
function read(core: Core): Envelope {
  const pointer = core.result_ptr(), length = core.result_len();
  bounds(core, pointer, length);
  // Copy before an await, an allocation, or a memory growth can invalidate it.
  const bytes = new Uint8Array(core.memory.buffer, pointer, length).slice();
  const result = JSON.parse(decoder.decode(bytes));
  if (!result || typeof result !== 'object' || Array.isArray(result) || typeof result.kind !== 'string')
    throw new Error('boundary-envelope');
  return result;
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
  let leased:Core|undefined;let reusable=false;
  try {
    const core = leased = acquire(module, imports);
    if (!(core.memory instanceof WebAssembly.Memory)) return internal();
    const [pointer, length] = write(core, input);
    let status = core.start(operation, pointer, length);
    let previousOperation = 0;
    let requestId: number | undefined;
    for (let steps = 0; steps < 64; steps++) {
      const output = read(core);
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
      const [resultPointer, resultLength] = write(core, result);
      status = core.resume(requestId, previousOperation, resultPointer, resultLength);
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
  let leased:Core|undefined;let reusable=false;
  try {
    const core = leased = acquire(module, imports);
    if (!(core.memory instanceof WebAssembly.Memory)) return internal();
    const [pointer, length] = write(core, input);
    let status = core.start(operation, pointer, length);
    let previousOperation = 0;
    let requestId: number | undefined;
    for (let steps = 0; steps < 64; steps++) {
      const output = read(core);
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
      const [resultPointer, resultLength] = write(core, result);
      status = core.resume(requestId, previousOperation, resultPointer, resultLength);
    }
    return internal();
  } catch { return internal(); } finally {release(module,leased,reusable,imports);}
}
