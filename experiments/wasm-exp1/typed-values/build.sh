#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
python3 experiments/wasm-exp1/typed-values/compiler/generate.py "${1:-experiments/wasm-exp1/compiler/build/projected/program.json}"
export RUSTFLAGS="-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144"
cargo build --offline --locked --manifest-path experiments/wasm-exp1/typed-values/compiler/Cargo.toml --target wasm32-unknown-unknown --target-dir experiments/wasm-exp1/typed-values/compiler/build/cargo --release
cp experiments/wasm-exp1/typed-values/compiler/build/cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm experiments/wasm-exp1/typed-values/compiler/build/application.wasm
