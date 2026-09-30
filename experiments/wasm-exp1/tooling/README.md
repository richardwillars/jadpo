# WASM-EXP1 common host/tool setup

This directory pins common experiment tools. The handwritten WAT in `smoke.mjs`
tests host packaging only; it is **not** a Jadpo compiler route, source fixture,
protocol implementation, correctness result, or performance measurement.

On 2026-09-30 the official npm `latest` metadata resolved to Wrangler **4.144.0**
and wabt **1.0.39**. `package.json` has exact versions; `package-lock.json` freezes
their dependency graph and registry integrity values. Wrangler requires Node >=22;
setup used Node **24.18.1** and npm **11.16.0**. The installed workerd reports
**2026-09-26** (package **1.20260926.1**). Versions, registry URLs/integrities,
selected executable hashes, manifest/lock hashes and disk measurements are in
`setup-evidence.json`. These are developer tools, not generated application runtime
dependencies. The installed tree occupies 220,872 KiB; regular-file logical lengths
sum to 355,853,390 bytes (hardlinked paths can count more than once).

From the repository root:

```sh
npm ci --prefix experiments/wasm-exp1/tooling --no-audit --no-fund --cache build/wasm-exp1/tools/npm-cache
node experiments/wasm-exp1/tooling/smoke.mjs
WRANGLER_SEND_METRICS=false WRANGLER_LOG_PATH=build/wasm-exp1/tools/dry-run.log experiments/wasm-exp1/tooling/node_modules/.bin/wrangler deploy --dry-run --config build/wasm-exp1/tools/smoke/wrangler.jsonc --outdir build/wasm-exp1/tools/bundle --env-file /dev/null
WRANGLER_SEND_METRICS=false WRANGLER_LOG_PATH=build/wasm-exp1/tools/types.log experiments/wasm-exp1/tooling/node_modules/.bin/wrangler types build/wasm-exp1/tools/smoke/worker-configuration.d.ts --config build/wasm-exp1/tools/smoke/wrangler.jsonc --env-file /dev/null
WRANGLER_SEND_METRICS=false WRANGLER_LOG_PATH=build/wasm-exp1/tools/local-dev.log experiments/wasm-exp1/tooling/node_modules/.bin/wrangler dev --local --config build/wasm-exp1/tools/smoke/wrangler.jsonc --port 19873 --inspector-port 19874 --persist-to build/wasm-exp1/tools/local-state --show-interactive-dev-session=false --env-file /dev/null
```

Installation was explicit. npm reported esbuild/workerd postinstall scripts pending
approval; no script approval was needed for the verified commands because the
platform binaries were available through installed optional packages. No autoinstall
is performed by the smoke script. `node_modules/` and `.wrangler/` are ignored.

Observed results:

- CLI versions matched the pins. Read-only `wrangler whoami --json --env-file
  /dev/null` exited 0, confirming CLI authentication was available. No login was
  attempted. Account output and the private temporary Wrangler log were discarded;
  only the status is retained.
- The 58-byte module imports only `host.identity` and exports `run`. The supplied
  host function returns its integer argument; `run(42)` returns 42 in Node.
- `deploy --dry-run` produced a JavaScript bundle and a separate Wasm module. It
  reported 0.59 KiB upload / 0.41 KiB gzip; these are smoke bundle figures only.
- Local `wrangler dev --local` returned HTTP 200 with
  `{"hostSmoke":true,"result":42}`. The process was stopped after the request.
- Type generation succeeded once the sandbox permitted a loopback listener.
  Wrangler printed a Node compatibility types hint, although this smoke explicitly
  adds no compatibility flags and imports no Node APIs into the Worker.
- No Worker was uploaded or deployed and no Cloudflare resource was created.

Raw non-sensitive results and generated files are under ignored
`build/wasm-exp1/tools/`. Initial attempts used a wrong metadata working directory,
`/dev/null` as a log directory, and a cwd-relative `../bundle` outside the workspace;
the corrected commands above succeeded. None of those failures tested Wasm support.

The configuration fixes the compatibility date to **2026-09-30**. This proves the
module import shape in local workerd and the Wrangler bundler only. It does not
establish deployed Cloudflare behavior, Rust-vs-direct-route feasibility, WASI,
threads, persistence, or performance. Those remain the coordinator's experiment.

Official references consulted:

- [Wasm in JavaScript](https://developers.cloudflare.com/workers/runtime-apis/webassembly/javascript/): import compiled modules and instantiate with explicit imports.
- [Wrangler Worker commands](https://developers.cloudflare.com/workers/wrangler/commands/workers/): `deploy --dry-run`, `dev --local`, and output options.
- [Wrangler general commands](https://developers.cloudflare.com/workers/wrangler/commands/general/): read-only `whoami --json` authentication check.
- [Workers best practices](https://developers.cloudflare.com/workers/best-practices/workers-best-practices/).

Setup occupied approximately 8 minutes of common host work, including approximately
30 seconds of tool/network wait. This is a bounded accounting estimate, not an
instrumented CPU measurement; it must be counted once in the experiment's shared
six-hour budget and excluded from comparisons between compilation routes.
