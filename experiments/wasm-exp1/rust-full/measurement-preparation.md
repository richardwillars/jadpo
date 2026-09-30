# Prepared matched-scope measurement commands

Prepared only; **not executed** during the coordinator's ongoing throughput
run. Wait for that run to end before using these commands. Existing
`../measurement/repeat-command.py` supplies repetitions and raw process wall/CPU
logs. `measure-build.py` supplies one matched logical pipeline: source check,
checked projection, target emission/build and one verified real-SQLite probe.
Both routes seed the same owner-scoped row and expect `success: alpha`.

Bun projection and backend emission occur in one common compiler invocation;
that combined stage is explicitly reported. Rust reports projection, Rust
generation and Cargo compilation separately. The total logical scope matches;
the implementation steps and target formats naturally differ. Bun emits
TypeScript and its fresh process performs runtime compilation during import.
Rust emits and compiles Wasm ahead of that fresh process. Do not compare a
subset of one pipeline against the whole other pipeline.

The private measurement subtree is `rust-full/build/matched-measurement/`.
Clean runs remove only their route's generated/projected/Cargo target folders;
installed frontend, toolchain, registry downloads and OS caches remain.
Incremental runs alternate a private copy's minimum length three/four. The
frozen source, deployed modules and original probe artifacts are not touched.
Run five clean samples immediately before five incremental samples per route
so the latter start from a warm original-source build. Separate mode counters
ensure the first incremental sample makes the three-to-four source edit.

```sh
python3 experiments/wasm-exp1/measurement/repeat-command.py --label full-rust-clean --count 5 --output experiments/wasm-exp1/rust-full/build/full-measurements -- python3 experiments/wasm-exp1/rust-full/measure-build.py --route rust --mode clean
python3 experiments/wasm-exp1/measurement/repeat-command.py --label full-rust-incremental --count 5 --output experiments/wasm-exp1/rust-full/build/full-measurements -- python3 experiments/wasm-exp1/rust-full/measure-build.py --route rust --mode incremental
python3 experiments/wasm-exp1/measurement/repeat-command.py --label full-bun-clean --count 5 --output experiments/wasm-exp1/rust-full/build/full-measurements -- python3 experiments/wasm-exp1/rust-full/measure-build.py --route bun --mode clean
python3 experiments/wasm-exp1/measurement/repeat-command.py --label full-bun-incremental --count 5 --output experiments/wasm-exp1/rust-full/build/full-measurements -- python3 experiments/wasm-exp1/rust-full/measure-build.py --route bun --mode incremental
python3 experiments/wasm-exp1/measurement/repeat-command.py --label full-rust-start --count 20 --output experiments/wasm-exp1/rust-full/build/full-measurements -- bun --no-install --env-file=/dev/null experiments/wasm-exp1/rust-full/startup.ts rust experiments/wasm-exp1/rust-full/build/probe.wasm experiments/wasm-exp1/compiler/build/projected/program.json
```

For matching Bun startup samples, pass `bun` followed by a frozen original
projection directory containing `bun/target/app.ts` and its `program.json`.
Do not accidentally use the last incremental variant if its minimum is four.

The fresh startup harness records external timing through the shared repeater,
internal compilation/import/invocation phases, process RSS, raw resource usage,
and actual final linear-memory pages for the common driver's exact Wasm
instance. It uses real disposable SQLite and includes schema/seed setup in
script total time. This is intentionally broader than the earlier probe's
in-memory host startup measurement. Process RSS includes the Bun host; it is
not standalone Wasm memory.

These scripts need their first execution and output review after throughput
work finishes. No timing results or successful-run claim is attached to this
preparation. Capture machine workload, source/tool hashes and clean/cached state
alongside the shared repetition logs before drawing comparisons.
