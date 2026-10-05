# Capability monitor performance ledger

This ledger is append-only. HTTP results remain the frozen Bun-hosted campaign;
the bridge entries below are attribution probes and must not be presented as
production capacity.

## Campaign contract

- Candidate: single-pass Rust monitor with generated route metadata and the
  typed WASM bridge; baseline: the frozen scoped-WASM monitor and the
  predecessor monitor artifact at `cf04716`.
- Primary measure: authorized-read end-to-end throughput and p95/p99 latency
  under the existing WAL/SQLite workload and concurrency protocol.
- Secondary measures: CPU microseconds/request, peak RSS, startup, clean and
  incremental build time, and SQL statement/transaction counts.
- Hard guards: same authentication and live revocation checks, checked input
  validation, generated SQL policy and freshness, fresh guest instances,
  redaction, failure mapping, transaction commit/rollback, and hostile-guest
  rejection. No cloud deployment, default-target switch or custom persistence.
- Provisional acceptance: at least 2x scoped-WASM authorized-read throughput
  with no conformance regression. If a valid end-to-end run cannot be made or
  the threshold is not approached, retain the candidate as experimental and do
  not replace the remaining JSON payload path.
- Budget and stop condition: one permitted end-to-end campaign with three
  paired repetitions per target, followed by at most three attributable
  optimization candidates. Stop at the first repeated environment blocker or
  after a candidate fails the acceptance threshold without a guardrail win.
- Execution settings: the task's model and reasoning effort are not exposed by
  the available task tools; no model override was made.

## 2026-10-01 — typed header and adapter buffer reuse

- Candidate: current uncommitted working tree, release WASM build, Rust 1.78.0,
  Bun 1.2.20, arm64 macOS.
- Baseline: checked-in predecessor `application.wasm` from commit `cf04716`;
  the run used the predecessor artifact saved before the candidate build.
- Hypothesis: moving status/request/operation/capability metadata out of JSON
  and reusing adapter buffers reduces bridge work without changing application
  payloads or host policy.
- Workload: operation 46 guest read, 50,000 sequential iterations, one pending
  `entity.read` frame and one completion per iteration, seeded row response.
- Command: `PREVIOUS_WASM=/tmp/jadpo-monitor-previous-application.wasm
  ITERATIONS=50000 bun experiments/capability-monitor/bridge-bench.mjs`.
- Evidence: `bridge-bench-results.json`.
- Result: mean elapsed time 502.103 ms → 453.248 ms (10.8% lower); serialized
  output 11,838,894 → 6,150,000 bytes (48.1% lower).
- Guardrails: 9 Bun conformance tests / 34 assertions, Rust workspace tests,
  generated-manifest pin check and source diff check all pass. Fresh guest
  instances, authentication, policy, redaction, failure handling and
  transaction rollback remain covered.
- Decision: keep as the current monitor candidate. Do not claim an HTTP gain
  until the loopback/SQLite campaign can run in a permitted environment.

## Resume checkpoint

The next candidate is the remaining JSON application payload encoding. It is
deferred until a valid end-to-end HTTP/SQLite measurement is available; the
typed-header result above is the baseline for that comparison. The production
recommendation remains unchanged: native for traditional hosting, WASM for
Workers pending a workerd-specific campaign.

## 2026-10-01 — in-process monitor confirmation

- Workload: 1,000 authorized reads after 50 warmups, three repetitions in each
  execution order, fresh SQLite file and fresh application guest per request.
- Command: `PREVIOUS_MONITOR_WASM=/tmp/jadpo-monitor-previous-monitor.wasm
  PREVIOUS_APPLICATION_WASM=/tmp/jadpo-monitor-previous-application.wasm
  bun experiments/capability-monitor/in-process-bench.mjs` (then repeat with
  `ORDER=current-first`).
- Evidence: `in-process-bench-results.json`.
- Result: pooled wall time 2,713.164 ms → 2,680.161 ms (1.2% lower) and CPU
  7,815.3 → 7,651.3 microseconds/request (2.1% lower). The result is within
  exploratory local variability and excludes HTTP/socket scheduling.
- Decision: retain the typed ABI, make no remaining-payload rewrite, and wait
  for a permitted end-to-end campaign.

## 2026-10-01 — prepared statement cache probe

- Candidate: monitor-local SQLite adapter with an opt-in prepared statement
  cache. The default monitor path remains uncached so this probe cannot change
  the frozen measurement path accidentally.
- Workload: 1,000 authorized reads after 50 warmups, three repetitions in each
  execution order, fresh SQLite file and fresh application guest per request.
- Command: `bun --no-install --env-file=/dev/null
  experiments/capability-monitor/sqlite-cache-bench.mjs` and the same command
  with `ORDER=cached-first`.
- Evidence: `sqlite-cache-bench-results.json`.
- Result: pooled throughput 366.49 → 373.11 requests/second (1.8% higher),
  mean CPU 7,763.3 → 7,706.3 microseconds/request (0.7% lower). This is an
  in-process probe and excludes HTTP/socket scheduling, workerd and native
  executable measurements.
- Guardrails: the monitor conformance suite, including its cache-enabled
  revocation and rollback case, covers authentication, validation, policy,
  freshness, redaction, failures and transaction recovery; the cache probe also
  checks the complete authorized response on every iteration.
- Decision: retain the cache only as an explicit experiment (`cacheStatements`
  must be passed as `true`), keep the default path unchanged, and do not claim
  this small local result as a production performance gain. The campaign stop
  condition is reached: this candidate does not approach the 2x scoped-WASM
  acceptance threshold and has no guardrail win.
