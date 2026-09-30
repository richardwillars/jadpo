#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."

target=all
if [ "${1:-}" = --target ]; then
  target=${2:-}
  if [ "$#" -lt 2 ]; then
    echo 'Usage: build.sh [--target rust|wasm|all] [program.json]' >&2
    exit 2
  fi
  shift 2
fi
case "$target" in
  rust|wasm|all) ;;
  *) echo "Unknown build target: $target (expected rust, wasm or all)" >&2; exit 2 ;;
esac
if [ "$#" -gt 1 ]; then
  echo 'Usage: build.sh [--target rust|wasm|all] [program.json]' >&2
  exit 2
fi

if [ "$#" -eq 1 ]; then
  projection=$1
else
  # Check the fixture from source without changing the frozen experiment outputs.
  cargo run --offline --locked --manifest-path experiments/wasm-exp1/compiler/Cargo.toml \
    --target-dir build/native-conformance/projection-target -- \
    experiments/wasm-exp1/fixture build/native-conformance/projected
  projection=build/native-conformance/projected/program.json
fi
python3 experiments/native-conformance/prepare.py "$projection"

if [ "$target" = rust ] || [ "$target" = all ]; then
  cargo build --offline --locked --release -p jadpo-native-conformance --manifest-path experiments/native-conformance/Cargo.toml --target-dir build/native-conformance/target
  echo 'Native executable: build/native-conformance/target/release/jadpo-native-conformance'
fi
if [ "$target" = wasm ] || [ "$target" = all ]; then
  RUSTFLAGS='-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --release -p jadpo-conformance-wasm --target wasm32-unknown-unknown --manifest-path experiments/native-conformance/Cargo.toml --target-dir build/native-conformance/wasm-target
  cp build/native-conformance/wasm-target/wasm32-unknown-unknown/release/jadpo_conformance_wasm.wasm experiments/native-conformance/build/application.wasm
  echo 'WASM module: experiments/native-conformance/build/application.wasm'
fi
