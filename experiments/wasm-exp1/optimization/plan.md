# WASM-EXP1 optimisation extension — 2026-09-30

The owner requested an optimisation investigation, prioritising latency and the
unexpectedly low SQLite throughput. This extends the completed experiment; its
original source, artifacts, results and decision remain historical evidence.
Production remains Bun. No new language feature or production migration.

Bound: 90 cumulative active engineer-minutes: attribution15, instance lifecycle35,
SQL/metadata10, correctness/cloud20, report10. Timing/build waits are separate.
Stop at the cap and report incomplete work honestly. No subagents required.

First separate raw SQLite, generic host adapter, instance creation, codecs, and
fresh Wasm core costs using the same frozen256-byte request and seeded database.
Measure existing code before selecting changes. Obvious candidates are instance
reuse with explicit reset, prepared-statement reuse, and immutable checked-plan
precomputation. Do not remove runtime validation or policy enforcement.

Attribution is descriptive: three rotations,0.2s warmup and1s measured perstage,
all outputchecked, no CPUjobs inparallel. Final paired comparison: five rotated
runs,1s warmup+3s measured, concurrency1and8, same256-byte input and synchronous
localSQLite. Preserve originalBun/freshWasm controls, fresh/pooled hostmock cases,
and isolate any additional SQLchange. Every timed and warmup result must pass.
These shorter durations define this extension and do not retroactively replace
the original40run benchmark. Record perrun throughput,p50,p95,p99,errorcounts;
no significance or productionHTTPcapacity claim.

A reused instance must have exclusive ownership until terminal completion;
reset must clear/drop request state and allocations, reject pending reset, use
fresh request handles and failclosed on stale resumes. Traps, bounds failures
and malformed protocol discard instances. Pool capacity is bounded; principal
context stays invocationlocal. Prove reset with adversarial sequential and
interleaved principals, failures, wrong handles, trap then validrequest, memory
bounds and the unchangedP/A runtime suites. ExistingA17/A18evidence cannot be
silently inherited for changed code: retain generator negatives, source mutation
and deterministic rebuild. Metadata optimisation must retain source/policy
identity checks and reject forged descriptors.

Prioritise the dominant measured cost. Stop after a useful isolated improvement;
a binary ABI, full typednative lowering, nativeFFI and HTTPload testing can be
future scoped questions. If a reusable candidate passes locally, repeat the fixed
cloud acceptance using a disposable Worker/SQLiteDO and clean it up. State which
host and driver path each result actually tests. Cloudperformance remains unmeasured.

Success is learning the bottleneck and quantifying verified improvements; no
promised speedup. Report whether the candidate reaches the original20%I/Op95 gate,
without hiding remaining differences or changing that gate. A faster result still
requires a separate backend-adoption decision.
