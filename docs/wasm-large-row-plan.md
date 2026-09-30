# Wasm large-row investigation and backend coverage plan

Status: two bounded passes complete, 30 September 2026. This is a separately scoped follow-up to the
[value-path experiment](wasm-value-path-results.md), requested by the owner.
The [row transport results](wasm-row-transport-results.md) cover attribution,
binary-row variants, local correctness/HTTP comparisons, current local startup and
validator diagnostics. Large-row parity failed; the primary small-probe regression
gate passed. The protocol below is retained as registered. General typed lowering,
the full coverage inventory and controlled/hosted qualification remain open. Bun
remains the working target; this pass does not select a backend.

The subsequent [immutable-row and write-attribution pass](wasm-typed-values-results.md)
adds modest large-read gains (about 3% throughput / 2% p95 / 3% CPU in concurrent
HTTP), while still failing Bun parity. SQLite timing localises the observed write
stalls mainly to native transaction completion and sometimes `UPDATE`. A separate
WAL/FULL comparison substantially improves both targets and puts the Wasm adapter
ahead on this fixture's write workloads. This qualifies neither sustained storage
behaviour nor a target switch; controlled-host/recovery and full coverage remain open.

## Question and starting evidence

Can the compiler remove the large-row transport penalty while preserving Jadpo's
validation, policy, failure and resource contracts?

The current candidate is SHA-256
`84cddcfff2fdfebd5eb5ef5013e2e1f8f9ee470cc3e51e6453551421aa85e90a`.
The 16 KiB row measured 16,829 isolated operations/s against Bun's 31,317;
concurrent HTTP measured 10,561 requests/s against 16,143, p95 2.6208 ms against
1.8143 ms, and server CPU/request 94.5 microseconds against 59.3. These are dated
local observations, not fixed acceptance targets for another machine.

Both targets already validate inputs and returned database rows. The generated
Bun `Item.read` calls `validate_Item`; its experiment wrapper also validates
entrypoint arguments. Wasm additionally checks its host protocol, and the current
storage adapter and generated guest both validate returned rows. Catalogue these
checks before claiming identical work or removing apparent duplication: they
protect different boundaries.

Today a returned row follows approximately this path:

`SQLite row → host validation → JSON stringify/UTF-8/copy → guest JSON parse →
guest validation/application → JSON serialization → copy/UTF-8/JSON parse in host
→ HTTP JSON serialization`.

Generic `serde_json::Value` trees, repeated encodings and copies are the leading
hypothesis, not a measured attribution of the entire gap. The older stage profiler
returned a small title from its probe; it must be extended to profile the actual
full-row return. Do not substitute its timings for that workload.

Wasm does not automatically accelerate validators. Measure already-loaded values
separately from values crossing the boundary. Unconstrained text type checks,
UUID checks, Unicode length constraints and record validation have different
costs. Adding authentication, policy and I/O to both targets may dilute a CPU win;
adding more computation inside the guest may amortise transport. Overall request
speed is an empirical outcome, not an assumed consequence of adding features.

## LR-1 — Establish comparable work and attribute the cost

1. Freeze the current module, generated Bun baseline, source/projection, toolchain
   and storage settings. Rerun paired baselines before changing the implementation.
2. Record where each target enforces input types, stored types, refinements,
   policy, result shape, resource bounds and faults. Any semantic mismatch becomes
   a correctness issue before a speed comparison. Diagnostic check-disabled probes
   may attribute cost but cannot qualify as candidates.
3. Extend profiling to distinguish SQL fetch, host row checks, host encoding,
   memory transfer/allocation, guest decode, guest validation, application work,
   guest encode, host decode, and HTTP response encoding. Use diagnostic guest
   counters or bounded component probes where external timers cannot separate
   phases. Record bytes transported, crossing counts, allocations where measurable,
   guest memory high-water and pool retention; identify unavailable metrics.
4. Separate three cases: fetch a large row and return a small field; fetch and
   return the full row; fetch and inspect/transform the large field. This separates
   inbound transport from outbound transport and actual application work.
5. Compare generated validators on valid and invalid values, with failure near
   the beginning and end of the record/text. Include type-only, UUID, Unicode
   min/max length and record checks. Report validation-only and complete-boundary
   results separately. Unsupported validators are coverage gaps, not substitutes.

Deliverable: a reproducible cost breakdown and ranked candidates. Instrumented
means are diagnostic; they must not be added to predict uninstrumented p95.

## LR-2 — Compare bounded implementation candidates

Implement candidates separately, choosing the first from LR-1's measured ranking.
Limit the first pass to the following three approaches rather than an open-ended
runtime rewrite:

| Candidate | Hypothesis | Required constraint |
| --- | --- | --- |
| A: fewer temporary buffers with the current JSON interface | Allocation and copies account for enough time to matter | Preserve ownership across suspension, memory growth and instance reuse; no unsafe borrowed views |
| B: generated typed JSON decoding/encoding | Generic value trees and field lookup are expensive | Generate from the checked schema, preserve unknown-field/null/refinement behaviour, avoid converting straight back into generic trees |
| C: generated typed row transport | Repeated JSON escaping/parsing dominates | Versioned internal frame with schema identity, explicit tags/nullability, bounded lengths and UTF-8 fields; validate on receipt |

Candidate C is the leading architectural experiment if the profile confirms the
transport hypothesis. Start with storage-row ingress, measure it, then add typed
row egress only if the result justifies it. Keep control envelopes unchanged
initially. Retain external HTTP JSON and the reference JSON implementation for
differential checks. This is an internal experimental interface, not new syntax
or a committed public ABI.

Use explicit ownership initially; do not make zero-copy a prerequisite. The
generated decoder may combine parsing and validation in one pass while preserving
all checks. Do not bypass guest validation, return unchecked raw host rows,
introduce result caches, or change query projections merely to improve the score.
Do not forward raw JSON directly to HTTP without proving output policy, response
shape, ownership and escaping equivalence. Such a shortcut is outside this pass.

Each candidate must work on renamed entities/fields and a source constraint
mutation. A fixture-specific fast path is not a compiler optimisation. If a
candidate shows no repeatable gain after one focused tuning pass, retain the
negative evidence and move on. If none closes the gap, report that result rather
than silently expanding into moving SQLite or the entire host into Wasm.

## LR-3 — Correctness and performance qualification

Use 256 B, 4 KiB, 16 KiB and 48 KiB text payloads, including ASCII, multibyte
Unicode, quotes, backslashes and control characters. Record actual encoded frame
sizes: all successful cases must fit the existing 65,536-byte frame limit,
including the envelope and escaping. Test just-below, exact-limit and oversized
frames separately; oversized cases must fail safely, not be silently truncated.
Include null/missing/extra fields, malformed UTF-8, lengths/offsets/tags, wrong
schema/version, forged policy, stale handles, traps, reset and interleaved users.
Do not add an unsupported multi-row query solely to populate this benchmark.

Preserve the existing probe/runtime, rollback, rejection, mutation/rebuild and
instance-reuse gates. Differentially compare values, authored failures, safe fault
events and database snapshots against Bun/reference behaviour. Keep the known
inner-fault attribution limitation explicit; neither hide it nor regress it.

Choose a candidate using isolated development measurements, freeze its hash, and
then run the HTTP qualification independently. Use sequential rotated runs, fresh
Bun baselines, concurrency 1 and 16, identical SQL settings, no retries and full
response verification. Qualification uses at least five paired repetitions with
2 seconds warmup and 10 seconds measurement per selected cell; retain raw results
and ranges, including losses. Avoid concurrent builds or other benchmarks.

Proposed experimental parity gate, fixed before candidate measurement:

- On the primary 16 KiB workload at concurrency 1 and 16, paired median
  candidate/Bun ratios: throughput at least 0.90; p95 and CPU/request at most
  1.10. Require at least four of five pairs inside these bounds for each metric.
- Small-read throughput, p95 and CPU must remain within 5% of the frozen current
  Wasm candidate by paired medians. Report whether its Bun advantage survives.
- Report p99/max, every payload class and memory retention; no errors, unbounded
  growth or silently excluded regressions. Other-size regressions above 10%
  require an explicit explanation and block a blanket large-row parity claim.
- If results are noisy or the no-work control has less than 2x throughput
  headroom, label HTTP capacity inconclusive. Isolated and CPU results remain
  useful, but rerun capacity with a separate load generator on a controlled host.

These tolerances define practical parity for this experiment, not statistical
significance, production capacity or an adoption gate. Repeat write smoke/tail
measurements to detect regressions; the existing write stalls remain a separate
unresolved adoption issue.

## LR-4 — Startup, hosting and decision

For the selected artifact, measure module compilation/instantiation, process to
ready, first successful HTTP response and warm response separately. Use at least
20 fresh processes per target, rotate order, and record filesystem/module cache
conditions, bundle size and memory. This replaces extrapolation from the earlier
46 ms Wasm / 87 ms Bun first-operation observation.

Qualify the exact new artifact on Cloudflare using the existing bounded hosted
correctness suite. Distinguish deployment-reported startup from measured requests
to fresh instances; if instance coldness cannot be established, say so. Controlled
server testing uses matching hardware/storage/runtime configuration and a separate
load generator; infrastructure provisioning and deployment are later execution
steps, not actions performed by this planning change.

Deliver a candidate decision, reproduced hashes, raw evidence and remaining gaps.
Retain Bun as the reference and working target until the independent coverage,
write behaviour and operational gates below are satisfied.

## COV-1 — Build the backend coverage inventory

Include the [HOST-1/HOST-2 capability review and probe plan](wasm-host-capabilities.md).
Separate language lowering from host functionality: a Worker can support the Wasm
instructions while lacking a requested filesystem, threading or process capability.
The initial Cloudflare/Bun matrix is documentation-backed; new hosted probes remain
unexecuted. Capability failures must be visible before deployment.

Before extending the backend broadly, inventory implemented language/runtime
contracts from the checked IR, Bun generator, compiler/runtime tests and golden
cases. For each capability record: Bun support; Rust/Wasm lowering; direct-Wasm
lowering; host dependency; positive/negative evidence; and the smallest next slice.
Use statuses `verified`, `implemented but unverified`, `unsupported`, and
`language contract unresolved`. Compilation alone never means verified.

Cover scalars/refinements/records/enums/collections; expressions/control flow and
calls; failures/diagnostics; queries/mutations/relations/migrations; transactions
and savepoints; authentication/principal freshness; row/field/invoke policy;
configuration/secrets/time; HTTP and external capabilities; suspension/cancellation
and resource budgets; and build/debug/deployment tooling. Distinguish missing Wasm
support from semantics that remain unresolved for every target.

## COV-2 — Qualify representative vertical slices

Order implementation from the inventory: (1) core values, control flow and failures;
(2) storage, policy and transactions; (3) real authenticated HTTP requests,
configuration/time and lifecycle; (4) the complete golden application once its
source/contracts are executable. Reuse the same semantic fixtures across targets,
including invalid data, cross-principal access, failures and rollback.

Production-auth comparisons must include equivalent fresh principal and credential
checks. Preserve the owner's query budget: one principal lookup, credential/session
checks counted separately. The current fixed-principal performance fixture does
not count as authentication coverage.

Services/jobs or other undecided contracts remain separately parked. Record
unexecuted golden cases openly. Rust-mediated versus direct generation stays a
separate route decision: gains on the Rust candidate do not establish direct-route
coverage. Default-target migration requires a further review of coverage, writes,
startup, host portability, build cost and diagnostics, not just the LR parity gate.
