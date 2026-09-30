// Portable acceptance harness. Storage and direct-role authorization are host
// capabilities; application validation, failures, and recovery execute in Wasm.
import { invoke, BUFFER_LIMIT, MEMORY_LIMIT } from './driver.ts';

type Json = any;
type Core = {
  memory: WebAssembly.Memory;
  alloc(length: number): number;
  start(operation: number, pointer: number, length: number): number;
  resume(request: number, operation: number, pointer: number, length: number): number;
  result_ptr(): number;
  result_len(): number;
};
type CaseResult = { id: string; status: 'pass' | 'fail' | 'not_run'; [key: string]: unknown };
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const equal = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
function requireThat(condition: unknown, reason: string): asserts condition {
  if (!condition) throw new Error(reason);
}
function operationId(acceptance: Json, name: string): number {
  const fact = acceptance.checked_semantic_facts.operation_name_spans.find((x: Json) => x.name === name);
  requireThat(Number.isSafeInteger(fact?.id), 'missing checked operation');
  return fact.id;
}
function writeRaw(core: Core, raw: string): [number, number] {
  const bytes = encoder.encode(raw), pointer = core.alloc(bytes.length);
  requireThat(pointer > 0 && Number.isInteger(pointer) && bytes.length <= BUFFER_LIMIT &&
    core.memory.buffer.byteLength <= MEMORY_LIMIT && pointer <= core.memory.buffer.byteLength - bytes.length,
    'invalid allocation');
  new Uint8Array(core.memory.buffer, pointer, bytes.length).set(bytes);
  return [pointer, bytes.length];
}
function read(core: Core): Json {
  const pointer = core.result_ptr(), length = core.result_len();
  requireThat(Number.isInteger(pointer) && Number.isInteger(length) && pointer >= 0 && length >= 0 &&
    length <= BUFFER_LIMIT && core.memory.buffer.byteLength <= MEMORY_LIMIT &&
    pointer <= core.memory.buffer.byteLength - length, 'invalid output bounds');
  return JSON.parse(decoder.decode(new Uint8Array(core.memory.buffer, pointer, length).slice()));
}
async function createCore(module: WebAssembly.Module): Promise<Core> {
  const core = (await WebAssembly.instantiate(module)).exports as unknown as Core;
  requireThat(core.memory instanceof WebAssembly.Memory, 'missing memory');
  return core;
}
function matches(actual: Json, expected: Json, hostCalls: number): boolean {
  return actual?.kind === expected.kind &&
    (!Object.hasOwn(expected, 'value') || equal(actual.value, expected.value)) &&
    (!Object.hasOwn(expected, 'host_calls') || hostCalls === expected.host_calls);
}

export async function runProbeSuite(
  module: WebAssembly.Module,
  acceptance: object,
  options: { mutatedModule?: WebAssembly.Module; projection?: Json } = {},
) {
  const spec = acceptance as Json;
  const cases: CaseResult[] = [];
  const probeId = operationId(spec, spec.scope.probe_entrypoint);

  async function execute(test: Json, selectedModule = module, pendingBarrier?: () => Promise<void>) {
    let hostCalls = 0;
    const hostProblems: string[] = [];
    const requests: Json[] = [];
    const principal = spec.seeds.principals[test.principal ?? 'owner'];
    const result = await invoke(selectedModule, probeId, test.input, async (capability, rawArgs) => {
      hostCalls++;
      const args = rawArgs as Json;
      // Record only fixture capability metadata, never the injected host error.
      requests.push({ capability, entity: args?.entity, semanticOperationId: args?.semanticOperationId,
        predicate: args?.predicate, principal: principal?.entity });
      try {
        requireThat(capability === 'storage.read', 'unexpected capability');
        requireThat(args?.freshness === 'authoritative', 'freshness mismatch');
        requireThat(args.semanticOperationId === operationId(spec, args.operation), 'operation metadata mismatch');
        requireThat(args.predicate?.operator === 'equal' && args.predicate.field ===
          spec.checked_semantic_facts.entity_model[args.entity]?.identity &&
          args.predicate.value === test.input.id, 'predicate mismatch');
        const policy = args.policy;
        requireThat(policy && policy.operation?.operation === args.operation &&
          Array.isArray(policy.operation.obligations) && policy.operation.obligations.length > 0 &&
          Array.isArray(policy.bindings) && Array.isArray(policy.entities), 'missing policy metadata');
        if (options.projection) {
          requireThat(equal(policy.operation, options.projection.policy.operations.find((x: Json) => x.operation === args.operation)) &&
            equal(policy.bindings, options.projection.policy.bindings) &&
            equal(policy.entities, options.projection.policy.entities), 'checked policy metadata mismatch');
        }
        const table = spec.seeds[args.entity];
        requireThat(Array.isArray(table) && principal, 'unknown authority or principal');
        let row = Object.hasOwn(test.host ?? {}, 'read_row') ? test.host.read_row :
          test.host?.read_missing ? null : table.find((value: Json) => value[args.predicate.field] === args.predicate.value) ?? null;
        // Direct-role storage restriction is derived from generated policy facts.
        // This bounded harness deliberately rejects other policy shapes.
        const obligations = policy.operation.obligations;
        requireThat(obligations.every((obligation: Json) => obligation.entity === args.entity &&
          obligation.effect === 'read' && obligation.origin === 'entity' && Array.isArray(obligation.subjects) &&
          obligation.subjects.length > 0), 'unsupported policy obligation');
        for (const obligation of obligations) {
          const rule = policy.entities.find((entry: Json) => entry.entity === args.entity);
          requireThat(rule && !rule.scopeField && obligation.subjects.every((subject: string) =>
            rule.rules.some((entry: Json) => entry.subject === subject && entry.effects.includes('read'))), 'policy rule mismatch');
          const bindings = policy.bindings.filter((binding: Json) => binding.entity === args.entity &&
            obligation.subjects.includes(binding.role));
          requireThat(bindings.length > 0 && bindings.every((binding: Json) => binding.scope === args.entity), 'unsupported policy binding');
          if (row && !bindings.some((binding: Json) => binding.principalEntity === principal.entity &&
            row[binding.field] === principal.id)) row = null;
        }
        if (pendingBarrier) await pendingBarrier();
        await delay(test.host?.delay_ms ?? test.host_delay_ms ?? 0);
        if (test.host?.fault) throw new Error(test.host.fault);
        return { kind: 'success', value: row === null ? null : JSON.parse(JSON.stringify(row)) };
      } catch (error) {
        if (!(test.host?.fault && error instanceof Error && error.message === test.host.fault))
          hostProblems.push(error instanceof Error ? error.message : 'host assertion failed');
        throw error;
      }
    });
    const sentinelAbsent = !JSON.stringify(result).includes('WASM_EXP1_SECRET_SENTINEL');
    return { actual: result, hostCalls, hostProblems, requests, sentinelAbsent };
  }

  for (const test of spec.probe_cases) {
    try {
      if (test.raw_input !== undefined) {
        const core = await createCore(module);
        const status = core.start(probeId, ...writeRaw(core, test.raw_input));
        const actual = read(core);
        cases.push({ id: test.id, status: status === 4 && matches(actual, test.expect, 0) ? 'pass' : 'fail', actual,
          abiStatus: status, hostCalls: 0 });
      } else if (test.interleaved) {
        const completionOrder: number[] = [];
        let entered = 0;
        let release!: () => void;
        const bothPending = new Promise<void>(resolve => { release = resolve; });
        let timer: ReturnType<typeof setTimeout>;
        const timeout = new Promise<void>(resolve => { timer = setTimeout(resolve, 2000); });
        const barrier = async () => {
          if (++entered === test.interleaved.length) release();
          await Promise.race([bothPending, timeout]);
          requireThat(entered === test.interleaved.length, 'interleave pending timeout');
        };
        const results = await Promise.all(test.interleaved.map(async (entry: Json, index: number) => {
          const result = await execute(entry, module, barrier); completionOrder.push(index); return result;
        }));
        clearTimeout(timer!);
        const expectedOrder = test.interleaved.map((_: Json, index: number) => index)
          .sort((a: number, b: number) => test.interleaved[a].host_delay_ms - test.interleaved[b].host_delay_ms);
        cases.push({ id: test.id, status: results.every((result: Json, index: number) =>
          matches(result.actual, test.interleaved[index].expect, result.hostCalls) && result.hostCalls === 1 &&
          result.hostProblems.length === 0 && result.sentinelAbsent) && equal(completionOrder, expectedOrder) ? 'pass' : 'fail',
          actual: results, completionOrder, expectedOrder });
      } else if (test.resume_mutations) {
        const outcomes: Json[] = [];
        for (const mutation of test.resume_mutations) {
          const core = await createCore(module);
          const initialStatus = core.start(probeId, ...writeRaw(core, JSON.stringify(test.input)));
          const pending = read(core);
          requireThat(initialStatus === 2 && pending.kind === 'pending' && Number.isSafeInteger(pending.requestId) &&
            Number.isSafeInteger(pending.operationId), 'resume test did not suspend');
          const row = spec.seeds[pending.args.entity].find((value: Json) => value[pending.args.predicate.field] === pending.args.predicate.value);
          const reply = { kind: 'success', value: row };
          if (mutation === 'duplicate resume after completion' || mutation === 'resume after internal failure') {
            const response = mutation === 'resume after internal failure' ? { kind: 'internal', message: 'Unexpected internal failure.' } : reply;
            const firstStatus = core.resume(pending.requestId, pending.operationId, ...writeRaw(core, JSON.stringify(response)));
            const first = read(core);
            requireThat(mutation === 'resume after internal failure' ? firstStatus === 3 && first.kind === 'internal' :
              firstStatus === 0 && first.kind === 'success' && first.value === row.title, 'first completion mismatch');
          }
          let status: number | null = null, actual: Json = null, trapped = false;
          const replyBuffer = writeRaw(core, JSON.stringify(reply));
          try {
            status = core.resume(pending.requestId + (mutation === 'wrong request ID' ? 1 : 0),
              pending.operationId + (mutation === 'wrong operation ID' ? 1 : 0), ...replyBuffer);
          } catch (error) {
            if (!(error instanceof WebAssembly.RuntimeError)) throw error;
            trapped = true;
          }
          if (!trapped) actual = read(core);
          const pass = trapped || (status === 3 && actual?.kind === 'internal');
          outcomes.push({ mutation, status: pass ? 'pass' : 'fail', abiStatus: status, actual, trapped,
            additionalHostCalls: 0 });
        }
        cases.push({ id: test.id, status: outcomes.every(x => x.status === 'pass') ? 'pass' : 'fail', actual: outcomes });
      } else if (test.source_mutation) {
        if (!options.mutatedModule) {
          cases.push({ id: test.id, status: 'not_run', reason: 'Requires separately rebuilt checked source mutation module.' });
          continue;
        }
        const before = await execute(test), after = await execute(test, options.mutatedModule);
        cases.push({ id: test.id, status: matches(before.actual, test.expect_before, before.hostCalls) &&
          matches(after.actual, test.expect_after, after.hostCalls) && before.hostProblems.length === 0 &&
          after.hostProblems.length === 0 && before.sentinelAbsent && after.sentinelAbsent ? 'pass' : 'fail', actual: { before, after } });
      } else {
        const result = await execute(test);
        const expectedCalls = test.expect.host_calls ?? 1;
        cases.push({ id: test.id, status: matches(result.actual, test.expect, result.hostCalls) && result.hostCalls === expectedCalls &&
          result.hostProblems.length === 0 && result.sentinelAbsent ? 'pass' : 'fail', ...result });
      }
    } catch {
      cases.push({ id: test.id, status: 'fail', reason: 'Harness assertion, malformed ABI output, or instantiation failed.' });
    }
  }
  return {
    schemaVersion: 1, experiment: 'WASM-EXP1', source: spec.source,
    scope: 'Frozen P01–P15 probe only; in-memory host capability, not SQL/transaction/authentication/full-slice proof.',
    publicOutputScope: 'Returned envelopes and harness report; no HTTP headers or external log sink inspected.',
    passed: cases.filter(x => x.status === 'pass').length, failed: cases.filter(x => x.status === 'fail').length,
    notRun: cases.filter(x => x.status === 'not_run').length, cases,
  };
}
