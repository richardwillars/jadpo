#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
DIRECT_PROJECTION="${1:-experiments/wasm-exp1/compiler/build/projected/program.json}"
DIRECT_OUTPUT="${2:-experiments/wasm-exp1/direct/build}"
DIRECT_ENTRY="${3:-probe}"
mkdir -p "$DIRECT_OUTPUT"
rm -f "$DIRECT_OUTPUT/probe.wasm"
python3 experiments/wasm-exp1/direct/generate.py "$DIRECT_PROJECTION" --entry "$DIRECT_ENTRY" --out "$DIRECT_OUTPUT"
test "$(rustc --version | cut -d ' ' -f 2)" = "1.78.0"
export RUSTFLAGS="-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144"
cargo build --offline --locked --manifest-path experiments/wasm-exp1/direct/Cargo.toml --target wasm32-unknown-unknown --target-dir experiments/wasm-exp1/direct/build/cargo --release
bun --no-install --env-file=/dev/null experiments/wasm-exp1/direct/link.ts experiments/wasm-exp1/direct/build/cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_direct_runtime.wasm "$DIRECT_OUTPUT"
