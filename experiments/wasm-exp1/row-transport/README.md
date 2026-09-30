# Schema-bound row transport experiment

This extends the frozen value-path candidate with an optional internal row format.
It does not change Jadpo syntax, HTTP JSON, SQL, authorisation, validators or the
default production backend. Both HTTP targets still run in the same Bun host.

## Selected implementation

The generator derives record field order, schema IDs, entrypoint result schemas
and a full SHA-256 projection digest. Row frames contain `JRW1`, the 32-byte digest,
a little-endian u32 schema ID, then fields in generated order. Tags distinguish
omitted, null, UTF-8 text, booleans and signed integers; text has a u32 byte length.
Unknown tags, malformed UTF-8, unexpected schema/digest, truncation and trailing
bytes reject. Existing generated application checks still enforce field types,
UUIDs and constraints. This is bounded flat-row transport, not full typed lowering
of every value or language feature.

The driver negotiates flags: 0 is reference JSON, 1 is row ingress, 2 is row
egress, 3 is both. Only storage-read success rows use binary ingress; updates,
control frames, failures and input arguments remain JSON. Egress applies to
record-valued successful entrypoints. Large-row eligibility requires a string of
at least 1,024 UTF-16 code units on ingress or 1,024 UTF-8 bytes on egress.
Small rows retain JSON. The selected HTTP candidate uses **egress only (flag 2)**.
Ingress did not improve the development measurements and is retained as a rejected
experimental variant, not the selected path.

The 65,536-byte binary limit and the original JSON-envelope size budget both apply.
The Rust size counter skips eight-byte chunks without escaping characters, with
exhaustive ASCII-position checks against serde JSON output. A binary frame that
cannot represent an otherwise valid JSON result falls back to JSON. No checked
value is silently truncated or normalised. Memory ownership, copying before
suspension, exclusive instance leases, reset, stale handles and bounded pooling
retain the existing protocol. The guest still uses generic `serde_json::Value`
internally; this tests transport rather than a fully specialised runtime.

## Reproduce

Run from the repository root using pinned Bun 1.2.20, Node 24.18.1 and Rust 1.78.
The existing checked projection, original generated Bun app and value-path module
must already exist. Benchmark workloads must run sequentially without builds or
tests in parallel. HTTP commands need permission to listen on loopback.

```sh
sh experiments/wasm-exp1/row-transport/build.sh
python3 experiments/wasm-exp1/row-transport/mutation-rebuild.py
python3 experiments/wasm-exp1/row-transport/negatives.py
bun --no-install --env-file=/dev/null test experiments/wasm-exp1/row-transport/codec.test.ts experiments/wasm-exp1/row-transport/reuse.test.ts experiments/wasm-exp1/boundary-http/cached-adapter.test.ts experiments/wasm-exp1/boundary-http/descriptor.test.ts
cargo test --offline --locked --manifest-path experiments/wasm-exp1/row-transport/compiler/Cargo.toml --target-dir build/wasm-exp1/row-transport/native-test -- --test-threads=1
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/run-cached.ts experiments/wasm-exp1/row-transport/compiler/build/application.wasm --mutated=experiments/wasm-exp1/row-transport/compiler/build/mutated.wasm
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/atomic.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/profile.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/profile-candidate.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/micro.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/micro.ts --selected
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/matrix.ts
node experiments/wasm-exp1/row-transport/http.ts --smoke
node experiments/wasm-exp1/row-transport/http.ts
node experiments/wasm-exp1/row-transport/http.ts --writes
node experiments/wasm-exp1/row-transport/startup.ts
python3 experiments/wasm-exp1/row-transport/validation-probe.py
bun --no-install --env-file=/dev/null experiments/wasm-exp1/row-transport/validation.ts
python3 experiments/wasm-exp1/row-transport/summarize.py
```

The full HTTP read qualification uses five rotated repetitions, concurrency 1/16,
2 seconds warmup and 10 seconds measured per cell. Its 80 cells include generated
Bun, previous Wasm, selected Wasm and no-work controls. Write checks are separate:
three rotations, concurrency 1/16, 1+3 seconds, 36 cells. All responses and final
snapshots are verified without retries. Eight preflight cases run before each
server's timing; startup.ts deliberately uses a separate server without preflight.
Authentication is a private fixture token and trusted owner, not production auth.

The supplemental size/escaping matrix uses shorter 0.1+0.5-second isolated runs.
Its CPU includes harness verification; do not compare that CPU with HTTP server
CPU. Escaped cells use fewer text characters to fit the original JSON budget;
actual frame bytes are recorded. Original and selected micro comparisons use
0.2+1-second runs and retain their earlier scheduling/verification overhead.

The validator module is a separate diagnostic build. It checks already resident
values in batches; Bun constructs validated values/throws errors whereas the guest
returns a boolean. Those results are not equivalent error-construction costs or
request-level speedups. Startup measurements use warm OS caches and temporary
SQLite initialization; they do not measure Cloudflare cold starts.

Initial binary variants lost and are retained with their source/artifact evidence.
The initial development `candidate` label meant both directions; final HTTP
`candidate` means selected egress. One HTTP qualification was deliberately stopped
to fix the binary/JSON size fallback before restarting on the corrected hash. A
sandboxed smoke attempt failed to listen before recording timed requests.

A pre-existing pinned-host limitation was reproduced with `SELECT ?`: Bun 1.2.20
SQLite binding strips an initial U+FEFF from a string. Transport-only tests preserve
it, including binary ingress/egress. This pass does not claim to fix that host bug.
The earlier oversized-host-result inner-operation diagnostic gap, write stalls,
broader backend coverage and hosted qualification also remain separate issues.
