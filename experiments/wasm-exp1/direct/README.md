# Direct Wasm probe

This experiment lowers the frozen checked projection into Wasm basic blocks,
lexical value registers, explicit suspension points, and recovery branches.
`generate.py` traverses expression and statement nodes and resolves checked
callable IDs. It does not generate Rust application functions or interpret the
application in JavaScript. Reachable calls are inlined; recursion and unsupported
reachable forms fail before a new artifact is emitted.

`runtime.rs` is a generic JSON/value helper compiled with Rust 1.78.0 and pinned
serde_json 1.0.133. It handles allocation, closed JSON envelopes, value handles,
Unicode scalar length, and compiler-supplied boundary type descriptors. It has no
application names, operation graph, continuation state, or recovery logic. The
same helper binary is used for the frozen source and constraint mutation.

`link.ts` uses pinned wabt 1.0.39 to convert this helper to WAT, append generated
application functions and constant data, and assemble one import-free Wasm
module. Existing numeric function indices remain stable. Constants occupy a
64-KiB buffer reserved by the helper. Internal helper exports are removed. This
textual linking is experiment infrastructure, not a production linker design.

The common ABI and runner are unchanged. Every invocation gets a fresh instance;
Wasm owns continuation state and guards invalid or repeated resumes. The host
handles the checked storage capability and direct-role restriction. Input and
stored row validation, required-read absence, declared recovery, and return
validation execute inside Wasm.

## Reproduce

Run from the repository root after building the checked projection and installing
the experiment's pinned tooling. No network installation occurs in these commands.

```sh
sh experiments/wasm-exp1/direct/build.sh
sh experiments/wasm-exp1/direct/mutation.sh
bun --no-install --env-file=/dev/null experiments/wasm-exp1/direct/probe.ts
bun --no-install --env-file=/dev/null experiments/wasm-exp1/direct/edge-cases.ts
python3 experiments/wasm-exp1/direct/test_generator.py
sh experiments/wasm-exp1/direct/source-variants.sh
```

The build script accepts projection, output-directory, and entrypoint arguments.
The default entrypoint is a CLI configuration; no application/entity names are
hardcoded in lowering. `source-variants.sh` checks and rebuilds independent source
copies with renamed entities/fields/callables and changed authored recovery
control flow. The frozen source is untouched. Rejected builds remove an older
`probe.wasm` first so stale output cannot be mistaken for a successful build.

The supported probe subset is required authoritative reads, lexical loads,
bindings, calls, attempts, returns, literals, and outcome matching. Query host
metadata comes from checked policy obligations. Mutation, transaction, recursive
calls, generalized expressions, and failure payloads remain rejected. This is
P01–P15 proof with a synthetic host; SQL, transaction, authentication and full
slice A01–A18 acceptance are outside its scope.

The Rust feature-disable flags produce unstable-feature warnings on Rust 1.78;
wabt validates the resulting module without threads, SIMD, bulk memory,
reference types, or multivalue. Probe and measurement evidence are separate in
`report.json` and `measurements.json`. No performance winner is inferred from a
small probe or uncontrolled local timing.
