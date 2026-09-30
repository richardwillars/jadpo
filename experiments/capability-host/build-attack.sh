#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
RUSTFLAGS='-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --release --manifest-path experiments/capability-host/Cargo.toml --target-dir build/capability-host/wasm-target -p jadpo-capability-host-attack --target wasm32-unknown-unknown
cp build/capability-host/wasm-target/wasm32-unknown-unknown/release/jadpo_capability_host_attack.wasm experiments/capability-host/build/attack.wasm
