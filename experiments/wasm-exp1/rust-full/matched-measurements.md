# Matched build and first-request observations

All 20 build samples and 40 first-request samples returned the expected result. Raw data, stage timings, source/tool hashes and exact commands are in `matched-measurements.json`; unedited logs remain under `build/full-measurements/`.

| Route | Operation | Samples | Median process wall | Range |
| --- | --- | ---: | ---: | ---: |
| rust | clean | 5 | 3704.850 ms | 3682.637–4414.691 ms |
| rust | incremental | 5 | 1050.100 ms | 1041.036–1529.641 ms |
| rust | first-request | 20 | 46.019 ms | 41.570–191.086 ms |
| bun | clean | 5 | 202.898 ms | 197.714–307.954 ms |
| bun | incremental | 5 | 194.205 ms | 190.518–375.188 ms |
| bun | first-request | 20 | 87.303 ms | 78.154–251.830 ms |

The build pipelines have the same logical scope: normal source check, checked projection, target emission/build and a verified real-SQLite probe. Bun projection/backend emission is one compiler step; Rust generation and compilation are separate. Process wall also includes private cleanup and wrapper overhead. See the JSON stage timings for narrower attribution.

Clean builds remove private generated/projected/Cargo target directories. Installed compiler/toolchain, package downloads and OS caches remain. Incremental runs alternate a private source constraint between three and four; the frozen source and deployed artifacts are untouched.

Private Rust clean build artifacts have a different hash from the deployed artifact because the Cargo lib source path differs (`generated.rs` versus `build/generated.rs`); panic source-path strings differ. All five generated Rust sources exactly match the frozen source hash, and all five private clean modules have the same hash. The first-request samples use the original frozen module and original Bun baseline, not private or incremental variants.

First-request samples include a new process, module import/compilation, SQLite schema/seed setup and one owner-scoped read. They are not duplicate readiness-only measurements. Rust instances finish at six linear-memory pages. RSS includes Bun and host glue, not just Wasm.

The coordinator confirmed the performance slot was free; measured workloads ran sequentially. Thermal state, power settings and OS caches were not isolated. These observations are not a causal speedup or long-term target recommendation. Reproduction commands are in `measurement-preparation.md`; the optional Bun first-request command is recorded verbatim in the JSON.
