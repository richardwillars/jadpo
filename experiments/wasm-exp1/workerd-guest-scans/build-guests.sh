#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
base=experiments/wasm-exp1/workerd-guest-scans/compiler
python3 "$base/generate.py" "${1:-experiments/wasm-exp1/compiler/build/projected/program.json}"
RUSTFLAGS='-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --manifest-path "$base/Cargo.toml" --target wasm32-unknown-unknown --target-dir "$base/build/scalar-cargo" --release
cp "$base/build/scalar-cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm" "$base/build/scalar.wasm"
RUSTFLAGS='-C target-feature=+simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --features vector --manifest-path "$base/Cargo.toml" --target wasm32-unknown-unknown --target-dir "$base/build/vector-cargo" --release
cp "$base/build/vector-cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm" "$base/build/vector.wasm"
RUSTFLAGS='-C target-feature=+simd128,+bulk-memory,-atomics,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --features vector --manifest-path "$base/Cargo.toml" --target wasm32-unknown-unknown --target-dir "$base/build/bulk-cargo" --release
cp "$base/build/bulk-cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm" "$base/build/bulk.wasm"
