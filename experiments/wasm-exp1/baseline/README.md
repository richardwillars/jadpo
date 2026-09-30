# Generated Bun baseline

Run from the repository root with the built checked-model projection compiler:

```sh
bun --no-install --env-file=/dev/null run experiments/wasm-exp1/baseline/run.ts
```

The runner performs fresh normal checking/projection with `--bun`, then imports
the generated callables, validators, operation capture and failure classes through
compiler-owned experiment exports. It does not rewrite generated application
behavior, interpret its statements, choose recovery arms, or replace persistence
with a handwritten application. All generated targets, databases, mutated source,
build logs and detailed results stay under ignored `build/wasm-exp1/baseline/`.
The frozen source digest is checked before and after mutation testing.

Normal cases use the generated SQLite persistence implementation with disposable
databases. The trusted adapter selects the declared owner/other principal and maps
its identity to the generated policy principal's `user_id` value. JSON parsing and
the generated validators run before any callable. A validation error thrown after
callable invocation is an internal fault, not invalid client input. Declared failure
classification uses the generated `DomainFailure` class, never a host object's
shape. Unexpected faults call the generated `reportRuntimeFault`; safe log capture
retains the generated event and discards no failing assertion.

Transparent wrappers record parameterized SQL, capability arguments, transaction
depth and completion order. The only injected persistence controls are asynchronous
delay, thrown fault, and malformed returned values. Real policy/storage execution
still supplies ordinary read/missing results. Host calls count generated required
read/update capabilities, not SQL statements, policy-context creation or transaction
bookkeeping. Seeding and snapshot queries are excluded from those counts.

The completed run reports **31 passing baseline equivalents**, **P14 not
applicable**, and **A17 not run**. Full outputs and before/after snapshots are in
`build/wasm-exp1/baseline/results.json`; each case inherits its report's source,
projection and generated application hashes and execution command. The compact
`summary.json` retains those identities without the large SQL/interleaving traces.

Coverage includes P01–P13 and P15, real SQL ownership predicates, omitted/null/empty
patch values, declared missing/conflict failures, transaction rollback and commit,
reopening SQLite, malformed host results, actual safe runtime logs, 100 interleaved
request pairs, delayed sequential writes, and a checked source mutation from minimum
length three to five. A clean repeat produces byte-identical projection, generated
application, persistence module and SQLite schema; compiler and Bun executable
hashes are retained. No native binary reproducibility claim follows from that.

Scope qualifications remain visible in the case records:

- P14's Wasm resume/request-ID/operation-ID protocol does not exist in this Promise
  baseline. A12 tests delayed write ordering and single completion; it does not
  claim a Bun equivalent of raw Wasm resume validation.
- A14 tests the safe adapter result and generated operational events, including a
  sentinel in both an error and trusted private principal context. The frozen source
  has no HTTP routes, so this callable baseline makes no HTTP response-header claim.
- A15 supplies the top-level checked operation ID to the existing generated fault
  reporter and resolves it to the frozen source range. It does not prove automatic
  innermost-call stack/source attribution.
- A17 covers Wasm backend capability rejection. Normal Bun supports constructs
  deliberately excluded from the experimental Wasm backend, so this runner does
  not fabricate a corresponding rejection result.

These results establish the common reference behavior and its adapter, not Wasm
route feasibility, cloud behavior, authentication, or production readiness.
