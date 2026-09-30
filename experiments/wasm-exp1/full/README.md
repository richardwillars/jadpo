# Full-slice host wiring

This directory supplies trusted test orchestration around an explicitly selected
compiler-produced Wasm artifact. It does not compile or interpret application
statements. `host.ts` delegates the whole synchronous `invokeSync` start/resume loop
to the generic checked-program storage adapter inside `runAtomic`. Storage outcomes
remain neutral; the generated core chooses authored failure/recovery arms.

## Local Bun + real SQLite

```sh
# With no artifact, records not_run without claiming a full-slice result:
bun --no-install --env-file=/dev/null run experiments/wasm-exp1/full/run-local.ts

# Complete artifact, checked projection and separately checked source mutation:
bun --no-install --env-file=/dev/null run experiments/wasm-exp1/full/run-local.ts FULL.wasm PROGRAM.json --mutated=MUTATED.wasm

# Explicit readonly-probe preparation mode:
bun --no-install --env-file=/dev/null run experiments/wasm-exp1/full/run-local.ts PROBE.wasm PROGRAM.json --probe-only --mutated=MUTATED.wasm
```

Every invocation creates a unique ignored directory under
`build/wasm-exp1/full-local/`, with its own SQLite authority, result JSON, actual safe
operational sink capture, and trap fixture. It clears no application database. A16
uses a newly opened readonly SQLite connection to observe committed/rolled-back
state. Single-parameter operations receive their value directly; multiple-parameter
operations receive the exact argument array. The trusted principal comes from the
frozen fixture by name, never from an application input field.

The portable P01–P15 harness intentionally retains its injected in-memory host
capability: those are its frozen asynchronous/malformed-host controls. It is labeled
separately from the real SQL A cases. The local runner additionally executes the
compiled readonly probe against real SQLite for owner/outsider isolation, a real
Wasm trap, and a valid request after that trap. This is infrastructure preparation,
not a full-slice acceptance pass.

## Cloudflare package, never deployment

```sh
PYTHONDONTWRITEBYTECODE=1 python3 experiments/wasm-exp1/full/build-cloud.py FULL.wasm build/wasm-exp1/full-cloud --program PROGRAM.json --mutated MUTATED.wasm
```

Use `--probe-only` when packaging the earlier probe. Output must stay below ignored
`build/wasm-exp1`. The builder copies the exact supplied Wasm/projection and trusted
host modules, records hashes, encodes the trap fixture with the pinned wabt CLI,
and emits a Worker plus a SQLite Durable Object migration/configuration. It never
installs packages or deploys resources. The coordinator owns any actual deployment.

The generated public Worker accepts only `GET /report`, executes fixed synthetic
cases, and chooses a fresh Durable Object authority for each report. Incoming body,
principal selectors, path segments, and query parameters cannot select application
inputs. Within a report, full-slice operations cross the private Durable Object RPC
boundary. `reopen()` is a separate `snapshot()` RPC after the operation, so A16 does
not merely inspect state inside the original transaction callback.

A12/A13 delays occur before the RPC is issued; no Promise or remote await crosses
`transactionSync`. The entire Wasm continuation loop and its SQL statements remain
synchronous inside one authority transaction. This tests an asynchronous outer
host boundary, not arbitrary asynchronous operations inside a database transaction.

## Fault evidence

The tiny handwritten WAT fixture exports the ABI and executes `unreachable` in
`start`. Both builders verify that invoking it raises `WebAssembly.RuntimeError`.
It is explicitly a host fault fixture, never an application compiler artifact or
compiler-route implementation. A13 selects this fixture for one request, then a
fresh instance of the real application module for the following request.

A15 records the semantic ID on the actual failed storage capability. It requires the
core's returned operation ID to equal that failed capability ID and resolve to its
checked source span/revision. An entrypoint fallback is visibly marked and cannot
satisfy A15. This closes the earlier test's ability to accept any mapped app span.
The host emits safe events through `emitEvent`; local runs capture that sink and the
Cloudflare package calls `console.error(JSON.stringify(event))`. Raw errors,
application inputs and the private sentinel context are never passed to that sink.
The coordinator must still inspect actual remote logs/response headers for the
corresponding external evidence; echoing an event in a report alone is insufficient.

A17 (negative capability/compiler inputs) and A18 (mutation/reconstruction) remain
separate evidence gates. A completed suite does not silently turn their `not_run`
records into passes. Public output and log checks explicitly retain their scope.

## Preparation observed

- Original Rust readonly probe: 15 portable probe cases passed, including its
  separately rebuilt mutation artifact; all 18 full-slice A cases explicitly not run.
- Generated readonly probe worked with the real SQL adapter for owner/outsider,
  actual trap containment (zero host calls), and a subsequent valid request.
- Cloudflare preparation bundle passed Wrangler dry-run. No cloud mutation or full
  Wasm-slice execution was performed in this preparation package.

Preparation consumed approximately six active minutes plus short tool waits,
charged to the shared full-slice budget. Raw results live under ignored build output.
