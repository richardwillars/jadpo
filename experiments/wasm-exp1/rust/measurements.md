# Rust probe measurements

These are local route-probe observations, not a comparison with Bun or direct
Wasm generation. Other experiment work ran on the same workstation. CPU load,
thermal state, power mode and OS caches were not controlled. Clean backend and
incremental measurements have different scopes, recorded below.

| Measurement | Runs | Minimum | Median | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Clean backend build | 5 | 3.477 s | 3.544 s | 4.227 s |
| Source-edit rebuild including frontend and semantic check | 5 | 0.985 s | 1.007 s | 1.551 s |
| Fresh Bun process, compile, instantiate, one probe, exit | 20 | 20.226 ms | 25.179 ms | 47.641 ms |

The median incremental stages were: semantic check 8.426 ms, checked projection
12.675 ms, backend generation/build 906.516 ms, and semantic startup check
16.099 ms. The wrapper's complete process time also includes setup and exit.
Individual raw stage timings are retained rather than inferred from totals.

Fresh processes reported median Wasm compilation of 1.126 ms and instantiation
of 0.149 ms. These internal timings exclude process launch/import overhead.
Every run returned `success: alpha` with one host call and ended at six Wasm
pages (384 KiB). The compiled minimum is five pages and maximum 128 pages.
Whole-process resident memory ranged from 33,931,264 to 34,521,088 bytes, with
median 34,349,056 bytes; this includes Bun and the JavaScript host and must not
be presented as standalone Wasm memory. Raw `process.resourceUsage()` is saved
without assuming its units.

`measurements.json` contains all normalized runs, phase timings, source/tool
hashes, binary paths, versions, cache/workload qualifications and artifact
verification. Unedited stdout/stderr and initial superseded runs remain in
`build/measurements/`. The initial incremental batch preceded explicit Bun
environment isolation and per-stage timing; it is excluded from summaries.
All reported Bun runs use `--no-install --env-file=/dev/null`.

## Reproduction

Run sequentially from the repository root. The checked projection and its
compiler must already exist. Cargo's download cache and shared frontend binary
remain installed; only the named Rust target directory is removed for each
clean sample, outside its timed region.

```sh
python3 experiments/wasm-exp1/measurement/repeat-command.py --label clean-build --count 5 --output experiments/wasm-exp1/rust/build/measurements --clean build/wasm-exp1/rust/cargo -- sh experiments/wasm-exp1/rust/build.sh
sh experiments/wasm-exp1/rust/build.sh
python3 -c "from pathlib import Path; Path('experiments/wasm-exp1/rust/build/measurements/incremental-state.json').unlink(missing_ok=True)"
python3 experiments/wasm-exp1/measurement/repeat-command.py --label incremental-build-sanitized --count 5 --output experiments/wasm-exp1/rust/build/measurements -- python3 experiments/wasm-exp1/rust/incremental.py
sh experiments/wasm-exp1/rust/build.sh
python3 experiments/wasm-exp1/measurement/repeat-command.py --label fresh-start-sanitized --count 20 --output experiments/wasm-exp1/rust/build/measurements -- bun --no-install --env-file=/dev/null experiments/wasm-exp1/rust/startup.ts
python3 experiments/wasm-exp1/rust/summarize-measurements.py
```

`incremental.py` changes a private source copy's minimum length between three
and four, runs the normal check and checked projection, rebuilds with the warm
Cargo target, and verifies the same valid payload. It never edits the frozen
fixture. The final original rebuild was verified byte-for-byte against SHA256
`8b85d7d65e04f546ab8adb1b25eb654f5c7802c175ea610caaf35e8d5eb71a13`.
The generator and runtime source hashes were unchanged by measurement work.

The measurement allowance began 2026-09-30 02:09:18 UTC. Actual completion and
estimated active/wait time are recorded in `measurements.json`.
