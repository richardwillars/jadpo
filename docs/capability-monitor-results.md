# Single-pass capability monitor results

This is the next bounded stage after the [capability-host experiment](capability-host-results.md).
The current WASM candidate remains frozen. The new monitor is a separate local
prototype and does not change the default target, the CLI, or the existing
authority experiment.

The trusted Rust monitor performs live credential/session/user checks, validates
the route input, starts a fresh application guest, accepts only the fixture's
declared `entity.read`/`entity.update` sequence, reconstructs the pinned SQL
plan, applies current row and field policy, redacts restricted fields, and binds
the guest completion to the row returned by SQL. It controls begin/commit/rollback.
The application computation runs once. The existing SQLite adapter is reused.

The prototype does not let a guest send SQL or transaction commands. A hostile
guest attempting raw SQL, commit, an unauthorized plan, a forged principal, or a
forged final projection is rejected; writes remain unchanged and transactions are
rolled back. Eight tests pass with 31 assertions. The fixture also checks
revocation before guest entry, ownership changes during a request, fresh guest
memory and private-field redaction.

The monitor's effect sequence and output projection are currently hardcoded for
the six fixture routes. That is the main compiler integration gap. The next
implementation should emit those bindings from checked lowering and reject any
route whose binding is absent. The monitor does not qualify instruction-fuel,
memory-exhaustion or disconnect cancellation isolation.

## Measurements

Three fresh-process repetitions used WAL, twelve clients, 128-request blocks and
300 ms per cell. `monitor` and `wasm` are Bun-hosted; `raw-wasm` is the frozen
previous WASM host; `native` is the existing native executable. The short cells
are attribution probes, and their full distributions are retained in
`build/capability-monitor/breakdown.json`.

| Workload | Native | Raw WASM | Scoped WASM | Single-pass monitor |
| --- | ---: | ---: | ---: | ---: |
| Missing auth req/s | 31,478 | 27,946 | 29,790 | 28,150 |
| Authorized read req/s | 10,868 | 7,427 | 1,203 | 1,235 |
| Denied pair req/s | 6,239 | 4,875 | 1,001 | 1,105 |

CPU microseconds/request were 95.5 / 217.0 / 1,822.9 / 1,666.7 for authorized
reads and 161.5 / 240.9 / 1,744.8 / 1,510.4 for denied pairs in the same target
order. This removes the reference application pass and saves about 9% CPU for
reads and 13% for denied pairs in this host, while throughput improves only
slightly. Fresh guest construction, JSON envelopes and Bun WASM scheduling still
dominate. The result supports a performance roadmap; it is not yet a performance
win large enough for production adoption.

## Performance roadmap and gates

Performance work is a required phase, with correctness as the gate for every
change:

1. Generate effect sequences, completion projections and route failure bindings
   from the compiler. Re-run all policy, mutation and hostile-guest checks.
2. Introduce a bounded binary bridge and allocation reuse. Compare p50/p95/p99,
   CPU, RSS, startup and SQL counts against the frozen JSON monitor baseline.
3. Evaluate a scrubbed guest instance pool only after proving memory/global reset
   and cross-principal isolation. Fresh instances remain the reference guarantee.
4. Measure SQLite scheduling and transaction overhead separately. Do not reduce
   freshness, validation, redaction or commit gating to improve a chart.

The prototype target is at least **2× the scoped-WASM authorized-read throughput**
with no conformance regression. Failure to reach that gate keeps native as the
recommended production direction and the monitor as a correctness experiment.

All WASM numbers here are Bun-hosted. A workerd result requires its own controlled
campaign and must not be inferred from these measurements.
