# Supplemental boundary, startup and inventory measurement

`boundary.ts` measures remaining common-host costs separately from the formal
application throughput runner. Do not execute it while another performance loop is
running. No throughput runner, application artifact, ABI, storage driver or full
suite is modified by this script.

```sh
bun --no-install --env-file=/dev/null run experiments/wasm-exp1/measurement/boundary.ts --wasm=experiments/wasm-exp1/rust-full/build/probe.wasm
# Static application component inventory only:
bun --no-install --env-file=/dev/null run experiments/wasm-exp1/measurement/boundary.ts --inventory-only
# Boundary tests without fresh-process startup samples:
bun --no-install --env-file=/dev/null run experiments/wasm-exp1/measurement/boundary.ts --skip-startup
```

Each invocation writes a uniquely named directory below ignored
`build/wasm-exp1/boundary/`. Results retain source/artifact hashes, raw repetition
samples and frame byte counts. The script verifies all measured component hashes
again at completion, failing if any input changed during measurement.

## Real compiled application frames

The actual frozen application allows arbitrary optional Text notes. The request
note and returned storage-row note therefore permit complete JSON input and host
success-response frames of **exactly 256, 4,096 and 65,536 bytes** while retaining
valid nominal IDs/titles. The output remains the authored `alpha` title. Every
request executes the complete generated Wasm input validation, named query,
stored-result validation and authored outcome match through the unchanged
experimental driver. The no-I/O host returns a valid row; it does not select an
application recovery arm.

These samples include all codecs, memory copies, fresh instance cost and one
storage capability crossing. They are not empty-crossing costs. Both synchronous
and asynchronous driver paths are sampled in 20 repetitions, after 50 warmups per
size/mode. Each repetition has 50/25/10 calls for the small/medium/large frames.
Every response and exact capability count is checked. Errors are retained as errors,
not counted as successful work. Pending/output frame sizes are also recorded.

## Explicit helper fixture

A separate WAT state machine implements only `start -> pending -> resume -> success`
and copies serialized bytes. It is generated with the pinned wabt package and its
bytes/hash are retained. It is **not** a Jadpo compiler artifact or alternate route,
and supplies no application validation, storage, policy or recovery behavior.

Its largest complete frame—the pending envelope—is exactly 256/4,096/65,536 bytes.
Request/response sizes are smaller because the pending envelope has additional
metadata; all exact lengths are reported. The fixture uses 256 KiB initial memory
and the frozen 8 MiB maximum. Each invocation gets a new instance. The dispatch is
an empty echo capability without I/O/delay. The host-only codec control assembles and
parses equivalent frames in JavaScript, but does not replicate Wasm allocation, so
its subtraction is not a precise isolated estimate of native call overhead.

Synchronous paths are timed without an added measurement-loop await. The asynchronous
path includes its real instantiation/dispatch awaits. Per-repetition min/median/p95/
max/mean and a summary of repetition medians are retained. These short microbenchmarks
are sensitive to JIT, GC and scheduling; no statistical significance or general Wasm
performance claim follows from the number of requests.

## Fresh-process readiness

Twenty fresh Bun processes per target run in alternating order. Parent and child
commands use `--no-install --env-file=/dev/null`; no local environment file is loaded
and no package autoinstall is allowed. Ambient `DATABASE_URL` and debug-stack flags
are removed before application loading; SQLite paths are disposable per child. The baseline imports
the actual generated app, which opens its disposable SQLite authority and creates
its schema. The Wasm counterpart compiles/instantiates the actual complete module
and initializes the checked SQL adapter/schema in a fresh disposable database.
Parent wall time includes the shared measurement harness startup; child readiness
starts after common harness imports. The full module is initialized but no HTTP
listener, application request or seeded data is involved.

These are process-fresh, **warm filesystem/OS cache** measurements, not machine cold
starts or Cloudflare isolate starts. The two readiness paths have different host
module-loading and storage implementations; the result is an end-to-end experiment
comparison, not an isolated engine-startup causal claim. Each child's observed RSS
is retained; it is not peak memory or per-request retained memory.

## Artifact and dependency inventory

Inventory records raw bytes, SHA-256 and individual gzip sizes for generated Bun
app/persistence versus the full Wasm module, complete checked projection, unchanged
ABI driver, traced full host and selected SQLite adapter. Components are unbundled
and no standalone HTTP entrypoint/test suite is counted; summed per-file gzip is
not a deployed transfer size. Both application slices use host builtins with no
external application npm dependency. Bun/SQLite/workerd binaries and compiler/dev
tools remain separate infrastructure costs. In particular, excluding those tools
does not mean building Wasm requires no dependencies.
