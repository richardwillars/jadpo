#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
test "$(rustc --version | cut -d ' ' -f 2)" = "1.78.0"
rm -f experiments/wasm-exp1/rust/build/probe.wasm
python3 experiments/wasm-exp1/rust/generate.py "${1:-experiments/wasm-exp1/compiler/build/projected/program.json}"
export RUSTFLAGS="-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144"
cargo build --offline --locked --manifest-path experiments/wasm-exp1/rust/Cargo.toml --target wasm32-unknown-unknown --target-dir build/wasm-exp1/rust/cargo --release
cp build/wasm-exp1/rust/cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust.wasm experiments/wasm-exp1/rust/build/probe.wasm
