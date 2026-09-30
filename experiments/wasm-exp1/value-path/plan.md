# Value ownership and speed-oriented Wasm build

Owner authorisation: continue optimisation and explore whether Wasm can beat Bun.
Start 2026-09-30 about 05:07 UTC. Bound: 60 elapsed engineer-minutes, one agent.
Keep earlier evidence and production unchanged. No paid/cloud resources needed.

Before adding ABI complexity, measure two low-complexity candidates: opt-level=3
instead of size optimisation; remove redundant owned JSON Value copies at terminal,
host-result and multi-argument boundaries. Keep JSON protocol, all checks, resource
bounds, policy, storage and exclusive leasing unchanged. Ownership transfers must
only occur after shape/identity checks, and after last use at the selected boundary.
Do not add arbitrary liveness assumptions about authored variables.

Build four modules from recorded source/flags: previous size build, speed-only,
ownership-only size build, combined speed+ownership. Reuse the previous checked
host adapter unchanged. Five rotated isolated repetitions, 0.2s warmup +1s timing
on the small probe and full 16KiB row; add escaped/Unicode text to correctness checks.
Choose the candidate on measured evidence, retain regressions rather than hiding
losers. Preserve normal compiler checks, source mutation, reproducible build,
unsupported-feature rejection and full probe/runtime/pool/policy tests.

After a useful validated improvement, run the prior HTTP protocol unchanged except
for the selected module: generated Bun, previous JSON-boundary candidate, new
candidate and matching no-work controls; 5 rotations, concurrency 1/16, 1s warmup
+3s measured, separate Node load process and Bun server. No measured retries. Record
CPU, tails, correctness and client limits. Do not run builds/tests while timing.
Use the same on-disk SQLite durability settings. Preserve aborted-run evidence.

Success includes an honest negative result. Require consistent paired evidence
before saying a candidate beats Bun; distinguish throughput, CPU and latency.
This local environment cannot establish an EC2 or Cloudflare win. A later EC2
comparison must pin CPU architecture/runtime, use non-burstable capacity and a
separate load host. EC2 improves experimental control, not relative speed by fiat.
