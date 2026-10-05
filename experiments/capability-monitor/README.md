# Single-pass capability monitor

This is a bounded follow-up to the independently enforcing capability-host
experiment. It keeps the checked Rust application in a fresh WASM instance, but
removes the trusted reference application execution. A trusted Rust monitor
authenticates, validates the route's effect sequence, reconstructs the checked
policy plan, runs the existing SQLite driver, redacts fields, binds the final
response to the row returned by SQL, and owns transaction completion.

The prototype generates a fixture-local effect sequence and output binding
manifest from the checked projection and contract. `build.sh` refreshes
`manifest.json` before compiling both WASM modules; the monitor rejects a route
whose checked binding is absent. A production compiler should emit the same
metadata as a first-class backend artifact rather than relying on this
fixture-local generator.

## Build and test

From the repository root:

```sh
bun run --cwd experiments/capability-monitor build:wasm
bun --no-install --env-file=/dev/null test experiments/capability-monitor/monitor.test.ts
node experiments/capability-monitor/smoke.mjs
node experiments/capability-monitor/measure.mjs
python3 experiments/capability-monitor/pin.py --check
```

To repeat the bridge-only comparison against the checked-in predecessor:

```sh
git show cf04716:experiments/capability-monitor/application.wasm > /tmp/jadpo-monitor-previous-application.wasm
bun experiments/capability-monitor/bridge-bench.mjs
```

The monitor and application modules are written to
`experiments/capability-monitor/monitor.wasm` and `application.wasm`. All WASM
measurements are Bun-hosted. No workerd or cloud deployment is part of this
prototype.

## Current result

The ten conformance tests include the typed ABI frame, live revocation and row
policy, transaction rollback, the cache-enabled revocation/rollback path, four
hostile guest modes, fresh guest memory and forged completion rejection. The
three-repetition follow-up uses twelve clients and 300 ms cells.
The persisted measurements are in `build/capability-monitor/breakdown.json`.
The checked-in copy is `results.json`.
The latest refreshed run reaches roughly 1,577 authorized reads/s and 1,366
denied pairs/s; scoped WASM reaches 1,434 and 1,248, raw WASM 7,792 and 4,950,
and native 17,907 and 10,949. Monitor CPU is about 1,309 microseconds/request
for reads and 1,280 for denied pairs, versus 1,504 and 1,458 for scoped WASM.
The short cells vary with local host load; the frozen capability-host campaign
remains the primary comparison.

The follow-up ABI slice uses numeric status/request/operation/capability fields
and reuses the WASM adapter's input/output buffers. In a 50,000-iteration guest
read microbenchmark, three runs averaged 502.103 ms for the previous full-JSON
frames and 453.248 ms for the typed-header/reuse build, a 10.8% reduction. The
serialized output fell 48.1% (11,838,894 to 6,150,000 bytes per run). The
application payload remains bounded JSON, so this is an ABI attribution result,
not an end-to-end HTTP throughput claim; the source data is in
`bridge-bench-results.json`.

The append-only experiment ledger and resume checkpoint are in
`performance-ledger.md`.

The in-process monitor confirmation is in `in-process-bench-results.json`; it
exercises the real auth/policy/SQLite path without opening a socket and is kept
separate from the HTTP capacity table.

The prepared-statement cache probe is in `sqlite-cache-bench.mjs`, with paired
results in `sqlite-cache-bench-results.json`. It improved this in-process cell
by 1.8% and reduced CPU by 0.7%; caching is therefore opt-in for the experiment
and remains disabled on the default monitor path. It is not a production
capacity result and does not justify changing the adapter contract.

The modest improvement is expected. The guest still has a fresh instance per
request and JSON monitor/guest round trips. The next performance work is
measured optimization behind the conformance gate:

1. Generate effect-sequence and output-binding metadata from the compiler rather
   than maintaining fixture-local tables.
2. Replace the remaining JSON payloads with a bounded binary representation only
   if an end-to-end measurement justifies the added adapter complexity.
3. Measure a scrubbed instance pool only if mutable globals and linear memory can
   be reset and proven not to retain caller data.
4. Measure host scheduling and SQLite driver batching without changing freshness,
   validation, SQL policy or transaction semantics.

The first performance gate is a twofold improvement over the scoped-WASM
authorized-read baseline while retaining all conformance and rollback checks.
The architecture should not be promoted if that gate requires guest memory reuse
without a demonstrated isolation proof.
