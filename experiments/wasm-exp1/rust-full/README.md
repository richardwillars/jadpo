# Provisional Rust full slice

This separate extension preserves the original `../rust/` probe. Both routes
passed the shared probes; the route comparison remains **inconclusive**. Rust
was provisionally selected to reuse rustc's async frame generation, not because
this implementation proves it is faster or the better long-term backend.

The generator consumes the same complete checked structural projection and
emits actual Rust call, return, binding, outcome and update control flow. All
six checked callables are exposed by their semantic IDs. One-parameter calls
take the direct JSON value; calls with multiple parameters take an array of
exactly the declared arity. Resolved parameter nullability, nominal constraints,
optional field presence and closed record shapes are validated inside Wasm.

Storage reads use the frozen probe protocol. Updates add checked `changes` to
the same authoritative predicate/policy descriptor and suspend with capability
`storage.update`. The host returns a closed success envelope containing a
neutral storage result: `{status:"found",row}` or a status of `missing`,
`conflict`, or `empty`. Generated code validates that exact result shape and
stored row, then selects only the authored declared failure. Unexpected host
results remain internal faults tagged with the actual pending semantic ID.

The host must execute the complete synchronous start/resume loop through
`storage.runAtomic`. Entity actions join that single transaction. Terminal
domain/internal/invalid outcomes force rollback through the adapter's private
exception marker; simply returning a domain result inside transactionSync
would incorrectly commit. The outer Durable Object RPC may be awaited, but no
Promise or external await belongs inside the synchronous SQL transaction.

The backend rejects cross-store plans, unavailable authoritative freshness,
handled nested mutation/savepoint scopes, recursive calls and unsupported
reachable constructs before emitting an artifact. It does not implement a
general migration system, external services, authentication adapters or
arbitrary Jadpo programs. Principals still enter only from the trusted test
boundary. Runtime allocation retains the probe's 64 KiB buffer and 8 MiB memory
bounds, one instance per request, no Wasm imports and panic-abort behavior.

## Reproduce

From the repository root, with the shared checked projection already present:

```sh
sh experiments/wasm-exp1/rust-full/build.sh
sh experiments/wasm-exp1/rust-full/mutation.sh
bun --no-install --env-file=/dev/null experiments/wasm-exp1/full/run-local.ts experiments/wasm-exp1/rust-full/build/probe.wasm --mutated=experiments/wasm-exp1/rust-full/build/mutated-probe.wasm
python3 experiments/wasm-exp1/rust-full/negative-tests.py
bun --no-install --env-file=/dev/null experiments/wasm-exp1/rust-full/negative-import.ts
python3 experiments/wasm-exp1/rust-full/rebuild-check.py
python3 experiments/wasm-exp1/rust-full/nullable-regression.py
bun --no-install --env-file=/dev/null experiments/wasm-exp1/rust-full/update-boundaries.ts
```

`build/manifest.json` lists every entry, source range, semantic ID, parameter
shape, checked source revision and whether mutations require an atomic host
boundary. The `atomic` manifest flag denotes that host requirement, including
entity actions; authored consistency remains in the checked projection.
Generated output is not repaired by hand. Compiler-owned helpers and generated
application code use the same pinned Rust/Cargo flags and dependency lock as
the initial Rust route.

## Evidence and known limitations

The local shared runner passes P01–P15 and A01–A16. A17 is recorded separately
as source/checked-lowering rejection cases plus an unapproved import host
rejection. A18 combines actual minimum-length source mutation and a clean
backend rebuild with identical generated source, manifest, lock and Wasm hash.
The shared test harness originally compared JSON object insertion order; the
coordinator corrected it to semantic object equality while preserving array
order and expected values.

Independent review found that the initial entry factory ignored the resolved
nullable flag on direct parameter references. The corrected factory now has
retained source-to-Wasm regressions for one and multiple parameters (eight
cases). The frozen fixture has no nullable entry parameters, so this correction
does not change either frozen artifact. Eleven additional update boundary
cases cover array arity, invalid patch fields, and malformed storage results.

Safe operation-ID faults resolve through checked source metadata, but Rust
compiler diagnostics and unsupported lowering still lack a polished Jadpo
diagnostic translation layer. Per-request instances and copied JSON values
favor clear ownership over optimization. No performance or production-readiness
claim follows from this slice; the coordinating report owns cloud evidence,
measurements and final target disposition.
