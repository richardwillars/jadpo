#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
base=experiments/wasm-exp1/workerd-shared-rows/compiler
python3 "$base/generate.py" "${1:-experiments/wasm-exp1/compiler/build/projected/program.json}"
RUSTFLAGS='-C target-feature=+simd128,+bulk-memory,-atomics,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --features vector --manifest-path "$base/Cargo.toml" --target wasm32-unknown-unknown --target-dir "$base/build/cargo" --release
cp "$base/build/cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm" "$base/build/shared.wasm"
