// Compiler-owned experimental host boundary. No application control flow here.
export const BUFFER_LIMIT = 65_536;
export const MEMORY_LIMIT = 128 * 65_536;
type Envelope = { kind: string; [name: string]: unknown };
export type Dispatch = (capability: string, args: unknown) => Promise<Envelope>;
type Core = {
  memory: WebAssembly.Memory;
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
export async function invoke(
  module: WebAssembly.Module, operation: number, input: unknown, dispatch: Dispatch,
  imports: WebAssembly.Imports = {},
): Promise<Envelope> {
  // Mutable state and memory belong to this invocation; only a module is shared.
  try {
    const instance = await WebAssembly.instantiate(module, imports);
    const core = instance.exports as unknown as Core;
    if (!(core.memory instanceof WebAssembly.Memory)) return internal();
    const [pointer, length] = write(core, input);
    let status = core.start(operation, pointer, length);
    let previousOperation = 0;
    let requestId: number | undefined;
    for (let steps = 0; steps < 64; steps++) {
      const output = read(core);
      if (status !== 2) {
        const expected: Record<number, string> = {0:'success',1:'domain',3:'internal',4:'invalid'};
        return expected[status] === output.kind ? output : internal();
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
  }
}

// The authority invokes this inside a synchronous SQL transaction. The outer
// RPC/host call is awaited by the caller; no Promise crosses this transaction.
export function invokeSync(
  module: WebAssembly.Module, operation: number, input: unknown,
  dispatch: (capability: string, args: unknown) => Envelope,
  imports: WebAssembly.Imports = {},
): Envelope {
  try {
    const core = new WebAssembly.Instance(module, imports).exports as unknown as Core;
    if (!(core.memory instanceof WebAssembly.Memory)) return internal();
    const [pointer, length] = write(core, input);
    let status = core.start(operation, pointer, length);
    let previousOperation = 0;
    let requestId: number | undefined;
    for (let steps = 0; steps < 64; steps++) {
      const output = read(core);
      if (status !== 2) {
        const expected: Record<number, string> = {0:'success',1:'domain',3:'internal',4:'invalid'};
        return expected[status] === output.kind ? output : internal();
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
  } catch { return internal(); }
}
