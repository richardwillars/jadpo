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

## Observed 2026-09-30 run

The exclusive slot ran after the formal throughput process exited and before the
matched-build measurements. Raw evidence is
`build/wasm-exp1/boundary/run-Fbl1ga/results.json`; `boundary-evidence.json` retains
its hash, the measured script hash, artifact identities and compact results.
The exact executed script is also retained beside the raw evidence.

All **3,400 measured actual-application invocations** and **5,100 measured helper /
codec-control invocations** returned expected values with exactly one host call;
there were zero semantic errors. Warmups are excluded from these counts. Both
complete actual-application input and host response frames reached exactly 65,536
bytes without being counted as rejected work.

Median of 20 repetition medians, milliseconds per actual application invocation:

| Complete input and host-response bytes | Synchronous driver | Asynchronous driver |
| ---: | ---: | ---: |
| 256 | 0.0790 | 0.0907 |
| 4,096 | 0.5558 | 0.0937 |
| 65,536 | 0.7363 | 0.1981 |

The corresponding helper medians were 0.0483 / 0.0544 ms at the smallest frame,
0.4526 / 0.0610 ms at the middle frame, and 0.4080 / 0.1216 ms at the largest frame
(sync / async). The helper's largest frame is its pending envelope; its complete
request/response frames are consequently slightly smaller. The non-monotonic helper
and sync/async differences show why these short allocation-heavy samples should
not be read as isolated native call costs. No ratio or helper subtraction is used
as a causal speed claim; the formal throughput run is the sustained-workload evidence.

Fresh-process startup medians over 20 processes per target:

| Target | Parent wall time | Child readiness | Observed RSS |
| --- | ---: | ---: | ---: |
| Generated Bun application + SQLite | 69.84 ms | 52.66 ms | 59,154,432 bytes |
| Full Wasm + checked SQL host | 23.51 ms | 6.70 ms | 44,326,912 bytes |

These include different real host initialization paths. The generated Bun module
loads its general persistence runtime; the Wasm path loads the experiment's narrower
host. Neither includes an HTTP listener or a seeded application request. All
processes use the same Bun binary and sanitized runtime flags; OS caches remain warm.

Component inventories:

| Component set | Raw bytes | Sum of individual gzip bytes |
| --- | ---: | ---: |
| Generated Bun app + persistence | 179,574 | 22,237 |
| Full Wasm + checked projection + traced host/SQLite adapters | 168,913 | 60,960 |
| Full Wasm + checked projection + traced host/Cloudflare adapters | 169,218 | 61,152 |

The compressed component totals make the different text/binary compressibility
visible. They do not include a deployment wrapper, test suite, host binaries or
compiler/dev dependencies, and are not an HTTP transfer-size prediction.

Preparation and analysis consumed approximately nine active minutes; about fifteen
minutes of waiting for the exclusive slot are accounted separately. The measured script run
completed in 4.4 seconds (excluding orchestration). No other performance process overlapped this run.
