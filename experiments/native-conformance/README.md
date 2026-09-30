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
