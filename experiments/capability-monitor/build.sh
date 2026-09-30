#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/capability-monitor
RUSTFLAGS='-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' \
  cargo build --offline --locked --release --manifest-path experiments/capability-monitor/Cargo.toml \
  --target-dir build/capability-monitor/wasm-target \
  -p jadpo-capability-monitor-authority -p jadpo-capability-monitor-guest \
  --target wasm32-unknown-unknown
cp build/capability-monitor/wasm-target/wasm32-unknown-unknown/release/jadpo_capability_monitor_authority.wasm experiments/capability-monitor/monitor.wasm
cp build/capability-monitor/wasm-target/wasm32-unknown-unknown/release/jadpo_capability_monitor_guest.wasm experiments/capability-monitor/application.wasm
