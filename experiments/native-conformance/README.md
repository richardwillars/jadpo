# Shared Rust conformance slice

Follow-up to the native experiment. Keep both prior experiments and the selected
WASM candidate frozen. This is a local conformance variant, not a replacement
performance candidate, production migration, or change to Jadpo's language.

Bounded scope:

- Extract a target-independent generated Rust crate with explicit request state.
  Exercise that same crate natively and in a new local WASM module.
- Preserve the documented fallback-conflict contract: any normalized constraint
  maps through the authored fallback; ordinary driver faults remain internal.
- Enforce host/completion budgets inside Bun's generated transaction callback so
  rejected oversized writes roll back, without editing the generated Bun program.
- Check independent suspended requests, stale handles, cancellation, thread
  transfer, driver faults, real SQLite transactions and existing fixture parity.
- Re-run checked source rename/refinement and unsupported-plan gates. Record
  new module hashes and archive results separately. Do not reuse old performance
  scores for changed artifacts or qualify workerd/cloud behavior from Bun hosting.

The existing failure contract is in `docs/failure-model.md` (fallback conflicts)
and `docs/grammar-v0.1.md` (normalized constraint violations). A new language
interpretation is unnecessary. Policy/authentication expansion, general savepoint
lowering, production HTTP and optimized WASM transport remain outside this slice.

## Results and reproduction

See the [completed conformance report](../../docs/shared-rust-conformance-results.md).
The original native performance campaign is not retimed or overwritten here.

Use the existing checked projection compiler and Bun baseline prerequisites from
`../wasm-exp1/baseline/README.md`. Rust dependencies are pinned by this workspace's
Cargo.lock. From the repository root:

```sh
sh experiments/native-conformance/build.sh
cargo test --offline --locked --manifest-path experiments/native-conformance/Cargo.toml --target-dir build/native-conformance/target
bun --no-install --env-file=/dev/null test experiments/native-conformance/wasm.test.ts experiments/native-conformance/wasm-sql.test.ts
python3 experiments/native-conformance/mutation.py
node experiments/native-conformance/run.mjs
python3 experiments/native-conformance/archive.py
```

Mutation restores and rebuilds the original afterward. Run these sequentially;
local HTTP tests require loopback-listener permission. All scratch databases/logs
stay under `build/native-conformance`; archives go only to this experiment.
`core` forbids unsafe code and has no database/HTTP dependency. `native` and `wasm`
both depend on that crate. The WASM ABI adapter has one slot per module instance;
its host must lease instances correctly. Core cancellation and rollback-on-drop
are tested, while HTTP disconnect cancellation remains outside this slice.
