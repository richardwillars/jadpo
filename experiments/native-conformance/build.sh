#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
python3 experiments/native-conformance/prepare.py "${1:-experiments/wasm-exp1/compiler/build/projected/program.json}"
cargo build --offline --locked --release -p jadpo-native-conformance --manifest-path experiments/native-conformance/Cargo.toml --target-dir build/native-conformance/target
RUSTFLAGS='-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --release -p jadpo-conformance-wasm --target wasm32-unknown-unknown --manifest-path experiments/native-conformance/Cargo.toml --target-dir build/native-conformance/wasm-target
cp build/native-conformance/wasm-target/wasm32-unknown-unknown/release/jadpo_conformance_wasm.wasm experiments/native-conformance/build/application.wasm
