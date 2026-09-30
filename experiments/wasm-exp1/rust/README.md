# Rust-mediated probe

This route lowers the frozen checked structural projection to Rust async
functions and compiles them to core Wasm using Rust 1.78.0. Application call,
binding, return and exhaustive outcome control flow is generated Rust. Host
JavaScript dispatches only storage capabilities; it does not interpret Jadpo
statements or choose recovery arms.

Local P01–P15 all pass, including the checked source mutation from minimum
length three to five. Additional boundary/memory tests and two native runtime
oversize tests pass. The same artifact passed P01–P15 on Cloudflare (LHR),
version `cc7f9663-afbb-4fca-b0f9-e2960318fe5e`; see
`../evidence/cloud-rust-probe.json`. This is not full-slice, SQL, transaction
or authentication evidence.

From the repository root:

```sh
sh experiments/wasm-exp1/rust/build.sh
bun --no-install --env-file=/dev/null experiments/wasm-exp1/rust/smoke.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/rust/edge-cases.ts
sh experiments/wasm-exp1/rust/mutation.sh
cargo test --offline --locked --manifest-path experiments/wasm-exp1/rust/Cargo.toml --target-dir build/wasm-exp1/rust/native --lib -- --test-threads=1
```

`build.sh` consumes `compiler/build/projected/program.json` by default; pass
another checked projection path as its first argument. The shared projection
must already have been produced by the experiment compiler. `mutation.sh`
creates a private source copy, performs the normal semantic check and checked
projection, then rebuilds a separate artifact. It does not edit the frozen
fixture or replace the primary artifact.

Primary artifact: `build/probe.wasm`. Mutated artifact:
`build/mutated-probe.wasm`. Entry selector is the checked semantic ID recorded
in `build/manifest.json`; its input is the direct authored input object.
Generated source, manifests, build logs and detailed local results remain
under ignored `build/`. `report.json` records their hashes and local findings.

All nominal/field/closed-shape validation executes inside Wasm. Text length
counts Unicode scalars. Omitted object fields remain omitted, and null values
remain null. Named reads suspend through Rust's generated async frame; each
request owns a separate instance, memory, future and copied JSON values.
The pending storage descriptor carries checked operation identity, query
freshness, predicate and policy metadata. The trusted host returns a closed
success envelope containing the stored row or null. Generated Wasm validates
the row and maps null to the authored missing failure. Host faults and malformed
envelopes never enter the authored recovery arm.

The runtime has no Wasm imports, no WASI and no asynchronous host stack
extension. It uses an explicit start/resume protocol, bounded buffers and
memory capped at 128 pages. Rust panic behavior is abort. The JSON dependency
is pinned to serde_json 1.0.133 with exact transitive versions/checksums in
Cargo.lock; builds use `--offline --locked`. Rust emits warnings for the
explicitly disabled experimental target-feature flags, recorded in build logs.

The reachable subset excludes mutations, transactions and conditional
statements. An accepted source copy containing an `if` statement passes normal
check/projection and is rejected by this generator before producing an
artifact. Diagnostics currently expose a Python exception with the unsupported
operation, rather than a polished Jadpo diagnostic. The runtime's unsafe static
state relies on the documented single-instance-per-request, non-reentrant
execution model. Performance, optimized frame reuse and broader language
coverage remain unmeasured.
