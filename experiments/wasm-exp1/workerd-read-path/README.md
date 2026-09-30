# Local workerd read-path experiment

Run Jadpo's generated WASM in Cloudflare's workerd/V8 runtime, with SQLite
Durable Objects and no Bun application runtime. Compare the previous WASM,
selected compact-query/receipt candidate, its typed-ingress alternative, generated
JavaScript, and a no-SQL response control on this same host.

This is a local exploratory comparison, not a deployment, production backend,
maximum-capacity test, hosted cold-start measurement, or comparison against native
Bun performance. Node runs development tooling and the external HTTP generator.
Cloudflare exposes a `process` compatibility global; its presence does not mean
the application executes in Node. The bundled application has no Bun/Node imports
or accesses, and preflight verifies that Bun is absent.

## Fairness and limits

The JavaScript arm uses the existing compiler-produced functions, validators and
failure handling. Bundling replaces its persistence import with a read-only test
port, removes its unreachable Bun CLI, and substitutes unused runtime provenance
constants. Both language targets use the same authority adapter, scoped SQL read,
host row checks and SQLite Durable Object. This is generated JS adapted to
workerd, not an already supported production JS backend. Source and bundle hashes
are retained. Neither arm includes the full authentication stack: fixtures supply
a trusted principal.

Each request crosses HTTP, Worker-to-Object RPC, and response JSON. One Object per
target/repetition isolates fixture state; it is not a production sharding design.
No-op retains RPC and response overhead but skips SQL/application validation.
The external generator checks every response; setup and final snapshots verify
database state and request counts. Tests cover invalid input, missing/unauthorised
rows, authored recovery, and immediate owner changes. Request bodies and fixture
size are bounded; a random local token protects the test endpoints.

The selected artifact and flags are frozen before timing. Mode 14 uses JSON
ingress; mode 15 adds direct typed binary ingress. Separate imported module
identities keep mode negotiation independent. The runtime pool counters are
shared by those two modes in each isolate; they demonstrate bounded retention,
not per-target memory attribution or total workerd RSS.

Five rotated repetitions, concurrency 1/16, 0.5 seconds warmup and 2 seconds
measurement, small probe and full 16 KiB row: 100 cells. This shorter protocol is
exploratory. The older Bun qualification uses longer intervals and different host
plumbing, so absolute throughput across those campaigns is not comparable.
No-op headroom below 2x prevents a maximum-capacity claim. No CPU/request or cold
start comparison is inferred from this run.

## Reproduce

Prerequisites: the baseline generated JS, checked projection, frozen previous
module, selected read-path module/metadata, and existing experiment tooling.
Versions used: Node 24.18.1, workerd 1.20260926.1, Miniflare
5.20260926.1-alpha, Wrangler 4.144.0, esbuild 0.28.1. Compatibility date:
2026-09-30. All bindings are local; do not deploy this fixture.

Run sequentially, with other builds/tests/benchmarks stopped:

```sh
node experiments/wasm-exp1/workerd-read-path/build.mjs
node experiments/wasm-exp1/tooling/node_modules/wrangler/bin/wrangler.js types --config build/wasm-exp1/workerd-read-path/bundle/wrangler.jsonc build/wasm-exp1/workerd-read-path/worker-configuration.d.ts
node experiments/wasm-exp1/workerd-read-path/runner.mjs --smoke
caffeinate -i node experiments/wasm-exp1/workerd-read-path/runner.mjs
python3 experiments/wasm-exp1/workerd-read-path/summarize.py
```

`caffeinate` is macOS tooling; omit it on other systems. It prevents idle sleep,
not lid closure. A failed campaign is retained, not retried request by request.
Miniflare is disposed in `finally`. SQLite version introspection is not exposed
by this platform; the pinned workerd version identifies the tested implementation.
