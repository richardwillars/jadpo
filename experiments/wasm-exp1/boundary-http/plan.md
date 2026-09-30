# Remaining boundary cost and local HTTP experiment

Authorised 2026-09-30 after the instance-reuse follow-up. Start approximately
04:33 UTC. Bound this extension to 60 cumulative engineer-minutes, one agent.
Retain the prior source, artifacts and evidence. Production remains Bun.

1. Profile the existing reused/cached path before selecting improvements. Use
   JavaScriptCore sampling plus isolated JSON, descriptor, validation and guest
   boundary measurements. Instrumentation is attribution only, never used for
   final timings. Profile both small and larger text values.
2. Test at most two local improvements with unchanged policy/type checks and
   exact rejection behavior. Candidates: avoid parsing constant JSON inside the
   guest; avoid sorting/stringifying the same descriptor shape on every host call.
   Keep JSON ABI and exclusive pooling. Do not remove validation or trust incoming
   policy assertions. Reuse immutable compiler-owned metadata only.
3. Repeat the existing local probe/full suite, malformed/forged descriptor tests,
   cross-principal reuse and source-mutation/rebuild checks for changed components.
   No Cloudflare deployment is required by this local HTTP extension; changed
   artifacts must not inherit the previous artifact's cloud pass.
4. Compare generated Bun, previous optimised Wasm, and the new candidate over
   loopback HTTP. One server process at a time, a separate load-generator process,
   same server implementation, database PRAGMAs, seeded data, wire format and
   trusted fixture principals. Exercise a small read, larger row read, single
   update and two-update atomic action. Keep production authentication out of this
   fixture and label it explicitly. Check every returned result and post-run data.
   Preflight outsider/invalid/missing/conflict/rollback behavior before timing.
5. Five rotated repetitions, concurrency 1 and 16, 1 second warmup and 3 seconds
   timed per workload/target. Record end-to-end p50/p95/p99, successful throughput,
   errors, server CPU and handler timing. A no-work HTTP control diagnoses client
   limits; no claim of maximum capacity if the client ceiling is close. These are
   closed-loop local measurements, not arrival-rate SLOs or deployed capacity.
   Run final timing sequentially without builds/tests or other timing jobs.
6. Retain raw results, hashes, versions and reproduction commands; report failed
   gates and limits. Stop at the cap or after one useful paired comparison, and
   recommend the next decision without a backend migration.

## Load-client adjustment before the final comparison

Two incomplete Bun-fetch attempts failed at a fresh no-work control warmup.
Keep their evidence separate. Use Node 24.18.1's built-in HTTP client for every
final target, with explicit keep-alive ownership, no retry and the same control.
No runtime/application policy changes are justified by this harness issue.
