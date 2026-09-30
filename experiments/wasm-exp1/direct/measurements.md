# Direct probe measurements

These local descriptive observations cover the frozen probe only. Other agents
were building the Rust full slice and integration on the same workstation.
CPU load, power, thermal state, filesystem caches and code caches were not
controlled. They do not establish a target or route performance winner.

| Measurement | Runs | Minimum | Median | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Clean backend build | 5 | 3587.659 ms | 3606.104 ms | 3959.414 ms |
| Source edit, frontend, backend and semantic startup | 5 | 337.402 ms | 347.532 ms | 349.643 ms |
| Fresh Bun process through completed probe | 20 | 19.531 ms | 21.528 ms | 22.191 ms |

Clean backend timing starts from the existing checked projection, removes only
`direct/build/cargo` before each sample, and includes WAT generation, the generic
Rust helper build, WABT conversion/linking and final Wasm validation. Download
caches and the compiled frontend remain installed. Cleanup is outside timing.

Source-edit samples alternate `min_length` between three and four in a private
source copy. They include the normal semantic check, checked projection,
warm backend build/link, and successful probe startup. Their median individual
stages were 8.416 ms check, 12.358 ms projection,
235.481 ms backend and 16.491 ms semantic startup.
The generic helper does not change across application source edits. Each stage's
raw time is retained; the wrapper total also includes setup and process exit.

Fresh-start samples use the exact Rust probe measurement workload: a new Bun
process compiles the frozen module, instantiates once through the common driver,
and completes one in-memory host read without artificial delay. All 20 return
`alpha` exactly once. Median internal compile time was
1.160 ms and instantiation
0.159 ms; these exclude process launch and imports.
Every instance finished at [7] Wasm memory pages. Whole-process resident
memory ranged from 34,062,336 to 34,553,856 bytes, median 34,201,600;
this includes Bun and host code and is not standalone Wasm consumption.
Raw `resourceUsage()` is preserved without assuming units.

The original artifact was restored and verified as
`4f91575377e515ed9ccb6835edffcf28a8706903f813faf1cced4f0dea7f28aa`.
The original source was unchanged. Tool versions, source hashes, per-run results,
per-stage times and cache qualifications are in `measurements.json`; unedited
stdout/stderr are in `build/measurements/`.

## Reproduction

```sh
python3 experiments/wasm-exp1/direct/measure.py
```

This performs five clean builds, five alternating source edits, restores the
original build, then performs twenty fresh starts. Every Bun subprocess uses
`--no-install --env-file=/dev/null`. Only this route's derived Cargo output is
removed; no database, service, credentials or external environment is used.
