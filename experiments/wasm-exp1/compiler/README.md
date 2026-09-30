# Common checked-model projection (WASM-EXP1)

This standalone crate is experiment setup, not either Wasm compilation route.
It uses the production `analyze_project` and rejects diagnostics from syntax,
semantic/policy/entity, type and failure gates. It traverses the checked AST and
reads the checked semantic graph, inferred types, failure effects, entity plans,
transaction plans and policy model. It does not parse source with regexes or
turn the graph's name inventory into executable behavior.

From the repository root:

```sh
cargo test --offline --locked --manifest-path experiments/wasm-exp1/compiler/Cargo.toml
cargo run --offline --locked --manifest-path experiments/wasm-exp1/compiler/Cargo.toml -- \
  experiments/wasm-exp1/fixture experiments/wasm-exp1/compiler/build/projected --bun
```

`program.json` is the common schemaVersion 1 input to both generators. It carries:

- Declaration and callable semantic IDs, stable semantic operation names,
  source-relative byte ranges, and the production checked-source revision.
- Closed record fields with separate optional presence and effective nullable
  types; nominal/field reference chains retain their complete constraints.
- Structural binding, return, rejection and conditional statements; literal,
  field load, call, constructor, attempt, exhaustive outcome match, binary/unary,
  required query and update expressions. Expression type facts come from the
  checked type result when present; null means no recorded fact, not unknown
  syntax accepted without checking.
- Exact checked failure contracts and callable failure/suspension sets.
- Explicit role bindings, entity matrices, operation obligations and protected
  field-read facts from PolicyModel, not inferred tenant/owner names.
- Authority stores, authoritative query plans and transaction disposition,
  nesting/failure, isolation and retry contracts from EntityModel.

The fixture's canonical entity dossiers are essential: its Item plans name the
primary store, and update_pair carries the checked atomic transaction contract.
User is an identity-bearing nonpersistent entity used by the trusted principal
fixture. Host code must not invent a writable User authority.

The deliberately bounded projection rejects unsupported declarations and forms
rather than publishing a partial artifact: routes, auth, config, authored test
fixtures, collection type arguments, tagged enum payloads, memberships, inverse
loads, compound constraints, generated lifecycle fields, derived stores,
receiver/revision operations, non-authoritative freshness, non-primary domains,
durable workflows, named arguments, general assignment/match statements,
create/delete and advanced query ordering/pagination/includes. Rejection is
conservative across every declaration, even an unreachable declaration. This is
a scoped experiment input contract, not a claim those features lack Bun support.

`--bun` calls production `derive_target` and writes its artifacts under `bun/`.
It only appends compiler-owned trusted test exports to app.ts:
`experimentCallables`, `experimentValidators`, `captureOperation`, `DomainFailure` and `ValidationError`. Existing
generated content is preserved byte-for-byte. Callable parameters remain the
production signature: authored arguments, operation context, then optional
persistence client. Validation exports are production validators; the harness
must invoke boundary validation and use isolated storage. These exports are
not production routes or authentication adapters and do not prove authentication.

A failed check/projection writes no new output. Existing output from an earlier
successful invocation remains unchanged; callers must require exit zero and
match checkedRevision before using artifacts. Generators must reject unsupported
IR themselves; structural projection is not proof of target implementation.
No Rust/WAT backend, AST interpreter, app-specific host branch or behavior repair
exists in this crate.

Three focused tests verify executable outcome/policy/transaction facts, inherited
nullable/optional fields, source-constraint mutation changing descriptors and
revision, and rejection of invalid checked source versus valid unsupported
syntax. They do not claim Wasm or cross-target semantic conformance.

Dependencies are production path crates plus exactly pinned serde_json 1.0.151
and the checked-in standalone Cargo.lock. Offline builds use the already present
Cargo cache; an empty machine still needs those locked build dependencies.
