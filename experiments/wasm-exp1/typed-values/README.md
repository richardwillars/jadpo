# Immutable typed-row experiment

This is a bounded follow-up to `row-transport`. The frozen reference is that
experiment's selected egress-only module, SHA-256
`b488c657e7184ed1c8689504941d5a3db8cd77e555fa800cbdb93405bb3956a9`.
The selected candidate is
`631ea8e20a3cc55936f857acefe3fe68ada60b83a7b737fc651a51fae40b8b91`.
Bun remains the default target. No source syntax, policy, SQL or public ABI changes.

## Representation and proof

After ordinary JSON host-envelope decoding and the existing generated entity
validator, eligible flat rows become immutable, schema-ordered owned scalars.
Whole-row references share ownership through `Rc`, avoiding deep text copies.
Generated schema metadata supplies names/order; source renaming and constraint
mutations are checked. Generic inputs, updates, control messages and unsupported
row shapes retain `serde_json::Value`. This is not complete typed lowering or a
specialised JSON parser. It borrows no text from exported guest memory.

An accepted JSON success envelope supplies a compiler-internal upper bound on
its canonical flat-row success envelope. Omitted/null values remain distinct;
canonical text/integer spellings cannot expand beyond the accepted JSON here.
The guest obtains this bound from the checked input length, never host metadata.
It is retained only with the immutable row. Returning the same row through the
existing binary egress can avoid rescanning JSON escaping. Materialising a JSON
value drops the proof; updates and unknown provenance use reference size checks.
Binary ingress carries no such proof and retains its exact JSON budget check.
The 65,536-byte frame limit and binary-overflow JSON fallback are unchanged.

## Protocol fixed before HTTP qualification

Five rotated isolated comparisons selected this implementation without a tuning
pass. The HTTP harness retains five repetitions, concurrency 1/16, 2 seconds
warmup and 10 seconds measurement for small-probe and full 16 KiB reads. Previous
Wasm now means the row-transport candidate above. Bun and no-work controls are
fresh measurements. Every response, preflight and final snapshot is checked.
The earlier parity gate remains: at least four of five pairs and paired medians
within 10% of Bun for large-read throughput, p95 and CPU; small-read paired median
regressions versus previous Wasm must stay within 5%. Retain losses, p99 and max.
Shared local workstation results do not establish production capacity.

Write qualification uses three repetitions with 1+3-second timings. A separate
instrumented write run brackets native Bun SQLite transaction callbacks, including
every statement. Its before/after intervals include native wrapper overhead and
must not be called measured fsync syscall time. Separate synchronous disk/memory
comparisons use five rotated repetitions and 0.2+1.5-second timings; memory is a
diagnostic control, not a durable replacement. SQLite disk settings remain DELETE
journal / FULL synchronous unless an explicitly labelled control says otherwise.
Timing is off for the ordinary performance qualification. No EC2 or Cloudflare
resources are provisioned; this is the same local host.

The disk/memory result motivated a subsequent journal control (`write-journal.ts`)
using DELETE/FULL versus WAL/FULL, five rotated repetitions, rollback preflight and
twenty clean database reopens. `journal-http.ts --writes` then confirms WAL/FULL
with both generated Bun and frozen Wasm candidates, without SQL instrumentation.
It uses the same three-rotation 1+3-second write protocol. This is a follow-up
storage-setting comparison, not a change to the ordinary read/write qualification.
SQLite reports version 3.39.5, fullfsync=0 and WAL autocheckpoint=1000 on this host.
Clean reopen does not qualify crash or power-loss recovery.

## Reproduce

Use Bun 1.2.20, Node 24.18.1 and Rust/Cargo 1.78.0. Existing checked projection,
generated Bun baseline and frozen row-transport module must exist. Run benchmarks
sequentially, without builds/tests in parallel. HTTP requires loopback permission.

```sh
sh experiments/wasm-exp1/typed-values/build.sh
python3 experiments/wasm-exp1/typed-values/mutation-rebuild.py
python3 experiments/wasm-exp1/typed-values/negatives.py
bun --no-install --env-file=/dev/null test experiments/wasm-exp1/typed-values/*.test.ts experiments/wasm-exp1/boundary-http/cached-adapter.test.ts experiments/wasm-exp1/boundary-http/descriptor.test.ts
cargo test --offline --locked --manifest-path experiments/wasm-exp1/typed-values/compiler/Cargo.toml --target-dir build/wasm-exp1/typed-values/native-test -- --test-threads=1
bun --no-install --env-file=/dev/null experiments/wasm-exp1/typed-values/run-cached.ts experiments/wasm-exp1/typed-values/compiler/build/application.wasm --mutated=experiments/wasm-exp1/typed-values/compiler/build/mutated.wasm
bun --no-install --env-file=/dev/null experiments/wasm-exp1/typed-values/atomic.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/typed-values/micro.ts --selected
bun --no-install --env-file=/dev/null experiments/wasm-exp1/typed-values/write-diagnostic.ts
node experiments/wasm-exp1/typed-values/http.ts --smoke
node experiments/wasm-exp1/typed-values/http.ts
node experiments/wasm-exp1/typed-values/http.ts --writes
node experiments/wasm-exp1/typed-values/http.ts --diagnostic-writes
bun --no-install --env-file=/dev/null experiments/wasm-exp1/typed-values/matrix.ts
node experiments/wasm-exp1/typed-values/startup.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/typed-values/write-journal.ts
node experiments/wasm-exp1/typed-values/journal-http.ts --writes
python3 experiments/wasm-exp1/typed-values/summarize.py
```

The copied harness/runtime files freeze the prior experiment while making this
candidate independently reproducible. The pre-existing oversized-host-frame
fault-location fallback and pinned Bun SQLite leading-BOM issue remain. Production
authentication, full language coverage and latest-artifact hosting are not covered.
