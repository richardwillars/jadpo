# Independently enforcing WASM capability host

Bounded local follow-up to the frozen authenticated Rust experiment. The same
checked Jadpo fixture generates one Rust application and contract. Native executes
it directly. WASM uses a trusted Rust authority instance plus an isolated application
instance. The host runs the generated application as a reference execution, checks
the guest's exact scoped effect sequence and completion, and owns SQL/transactions.
It executes database effects once. This intentionally duplicates application
computation as a strict prototype, not an optimized production architecture.

The guest gets no credentials, configuration secrets, clock, database handle or
SQL capability. Restricted nullable fields are replaced by null before delivery
when the authenticated caller lacks field-read permission. Each invocation gets a
fresh application instance/memory and an authority-owned request scope. The host
compiles the application module once and creates instances after authentication.
The authority module and external contract are pinned by the trusted local WASM build.
A native-only rebuild does not update that lock; if it changes the contract, the
old WASM bundle refuses to start until rebuilt together.
Guest code may be replaced without acquiring authority to choose SQL or commits.

## Build

From the repository root, with an updated Jadpo CLI:

```sh
jadpo build experiments/capability-host/fixture --target native
jadpo build experiments/capability-host/fixture --target wasm
```

Without the CLI or Bun:

```sh
sh experiments/capability-host/build.sh native
sh experiments/capability-host/build.sh wasm
```

Inside this experiment directory, `bun run build:rust` and `bun run build:wasm`
provide the short aliases. Native runs without Bun. The WASM build creates
`build/authority.wasm`, `build/application.wasm` and `build/trust.json`; all three
belong to one reviewed build. Native is at
`../../build/capability-host/target/release/jadpo-capability-host-native`.
Existing root build aliases/defaults and frozen experiments are unchanged.
Dependencies are pinned and fetched from the existing offline Cargo cache.

## Correctness and adversarial checks

Run from the repository root, sequentially:

```sh
sh experiments/capability-host/build.sh
sh experiments/capability-host/build-attack.sh
bun --no-install --env-file=/dev/null experiments/capability-host/seed.ts
bun --no-install --env-file=/dev/null test experiments/capability-host/authority.test.ts
cargo test --offline --locked --manifest-path experiments/capability-host/Cargo.toml --target-dir build/capability-host/target -p jadpo-capability-host-native
python3 experiments/capability-host/mutation.py
node experiments/capability-host/run.mjs
python3 experiments/capability-host/freeze.py
```

The separate attack build is a deliberately hostile test WASM module, never a
normal build/deployment output. Tests also inject guest protocol faults, including
premature completion after a real first write and traps during cleanup. Mutations
change an authored refinement and field update grant on all targets and restore
both WASM modules/contract by hash. Nonnullable protected fields are rejected by
this lowering slice: silently inventing a value to redact them is not supported.

SQL/HTTP/application completion budgets stay at 64 KiB, request bodies at 32 KiB.
Only the private inter-module bridge allows 128 KiB for additional envelope framing;
it must not silently shrink the original row/completion capacity. Near-limit rows
are compared against the frozen raw-WASM module. Native/authority still validate
before commit. Handled nested savepoint recovery remains unsupported.

## Registered performance protocol

Four targets: generated Bun, new native Rust, frozen raw-SQL WASM, and the new scoped
WASM host. All WASM runs in Bun; no workerd or cloud run. The same synthetic seed,
SQLite settings, response assertions and SQL-count checks apply to all targets.
Three repetitions rotate the four-target order (not fully counterbalanced), with
seven workloads, WAL/DELETE and concurrency 1/12: 336 measured cells. Each fresh
process/database gets 32 warmups followed by at least one second in 128-request
blocks. SQL trace drains occur outside timed blocks; inclusive throughput is also
retained. CPU includes drains, RSS is sampled externally and process-lifetime peak
RSS comes from macOS `time -l`. Startup uses ten fresh processes per target.

The prior measurement harness is copied with target/path changes; sources/artifacts
are pinned before this campaign. Short warmups, JIT/GC transients, a local shared
client/server host and serialized database execution limit generalization. Keep
SQL differences (Bun pre-reads/savepoints versus guarded Rust UPDATE RETURNING)
separate from runtime effects. No retries or response/freshness caching.

```sh
python3 experiments/capability-host/pin.py
node experiments/capability-host/measure.mjs --smoke
node experiments/capability-host/measure.mjs
node experiments/capability-host/measure.mjs --startup
```

On subsequent runs use `pin.py --check`; do not overwrite a differing pin. Seed
credentials last one hour; reissue before later campaigns. Local listeners need
loopback permission. Keep tests/builds out of timed campaigns.

## Trust limits

This prototype strengthens confidentiality/integrity against guest effects and
completion forgery; it does not qualify availability isolation. Synchronous WASM
has no instruction fuel/deadline here, and arbitrary replacement modules have not
been qualified against CPU/memory exhaustion. The built guest has an 8 MiB memory
ceiling; that is not a memory limiter for every hostile replacement. Host/runtime,
compiler, authority artifact/lock, configuration and database driver remain trusted.
The SHA pin is a local admission check, not deployment signing or key management.
The localhost servers contain harness controls and must not be deployed.

After the timed campaigns, run the isolated instance-cost diagnostic and build timings:

```sh
bun --no-install --env-file=/dev/null experiments/capability-host/instantiation.ts
python3 experiments/capability-host/build-cost.py
```

`python3 experiments/capability-host/build-isolation.py` tests native-only build
separation with a real source mutation and restores the original artifacts. Run
these sequentially. `summarize.py` produces the aggregate JSON/table and
`archive.py` verifies and packages the evidence. The pre-build-fix recipe/manifest
are retained; measured runtime artifacts did not change in that correction.
