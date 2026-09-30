#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
target=${1:-all}
if [ "$#" -gt 2 ]; then echo "Usage: build.sh [native|rust|wasm|all] [fixture-directory]" >&2; exit 2; fi
source=${2:-experiments/auth-policy/fixture}
case "$target" in rust|native|wasm|all) ;; *) echo 'Usage: build.sh [native|rust|wasm|all]' >&2; exit 2;; esac
cargo run --offline --locked --manifest-path experiments/auth-policy/compiler/Cargo.toml --target-dir build/auth-policy/compiler -- "$source" experiments/auth-policy/build/projected --bun
python3 experiments/auth-policy/prepare.py
if [ "$target" != wasm ]; then
  cargo build --offline --locked --release --manifest-path experiments/auth-policy/Cargo.toml --target-dir build/auth-policy/target -p jadpo-auth-policy-native
  echo 'Native: build/auth-policy/target/release/jadpo-auth-policy-native'
fi
if [ "$target" != native ] && [ "$target" != rust ]; then
  RUSTFLAGS='-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --release --manifest-path experiments/auth-policy/Cargo.toml --target-dir build/auth-policy/wasm-target -p jadpo-auth-policy-wasm --target wasm32-unknown-unknown
  cp build/auth-policy/wasm-target/wasm32-unknown-unknown/release/jadpo_auth_policy_wasm.wasm experiments/auth-policy/build/application.wasm
  echo 'WASM: experiments/auth-policy/build/application.wasm'
fi
