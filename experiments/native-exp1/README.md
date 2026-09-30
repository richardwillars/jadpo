# Bounded native Rust experiment

Decision protocol, recorded before implementation measurements. No production
target change, language extension, cloud deployment, or new persistence engine.
The selected workerd SIMD/bulk WASM and its compiler remain frozen; hashes are in
`evidence/freeze.json`. The unselected shared-row experiment is not the baseline.

Compile the identical generated Rust from the guest-scans compiler natively.
Use rusqlite and SQLite, keeping generated validators, authored application control
flow, failure selection and immutable values shared. Native host code owns database
policy enforcement, transaction scope, HTTP and lifecycle. Do not use WASM's i32
pointer exports on a 64-bit host. Restrict execution to a single synchronous
application invocation at a time because the inherited runtime uses globals.

Before timing: differential fixture correctness, invalid inputs/stored rows,
ownership freshness, omission/null, conflict/missing rollback, safe internal
faults, clean reopen, source rename/refinement rebuild, unsupported-plan rejection.
Retain exact work and SQL differences from generated Bun. Fixed trusted principals
are fixture context, never production authentication evidence.

Measurements: fresh processes, sequential rotated native/Bun pairs, five repetitions,
concurrency 1/16, read-small/full 16 KiB/Unicode and single/pair writes; disk WAL/FULL
primary and DELETE/FULL write control. Use 0.5 s warmup + 2 s measurement per cell
for this bounded exploratory campaign (shorter than prior 2+10 s qualification).
Check every response, final snapshots and request counts; no request retries.
Record p50/p95/p99/max, throughput, process CPU/request, resident memory and
no-work headroom. Add isolated calls and 20 rotated startup processes per target,
clean/incremental build timing. Retain raw cells, losses and failed attempts.

Recommendation criteria: equivalent tested semantics are mandatory; repeatable
native CPU/read improvements and shared application code justify a further bounded
slice, not migration. Lack of 2x no-work headroom blocks capacity claims. Report
coverage, adapter duplication, SQLite versions, durability/recovery limitations,
warm OS caches and same-machine generator contention. Prior Bun-hosted WASM and
workerd measurements remain separately labelled historical evidence.
