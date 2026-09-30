#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
target=${1:-all}
case "$target" in native|rust|wasm|all) ;; *) echo 'Usage: build.sh [native|rust|wasm|all] [fixture-directory]' >&2; exit 2;; esac
[ "$#" -le 2 ]
source=${2:-experiments/capability-host/fixture}
cargo run --offline --locked --manifest-path experiments/auth-policy/compiler/Cargo.toml --target-dir build/auth-policy/compiler -- "$source" experiments/capability-host/build/projected --bun
python3 experiments/capability-host/prepare.py
if [ "$target" != wasm ]; then
 cargo build --offline --locked --release --manifest-path experiments/capability-host/Cargo.toml --target-dir build/capability-host/target -p jadpo-capability-host-native
fi
if [ "$target" != native ] && [ "$target" != rust ]; then
 RUSTFLAGS='-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144' cargo build --offline --locked --release --manifest-path experiments/capability-host/Cargo.toml --target-dir build/capability-host/wasm-target -p jadpo-capability-host-wasm -p jadpo-capability-host-guest --target wasm32-unknown-unknown
 cp build/capability-host/wasm-target/wasm32-unknown-unknown/release/jadpo_capability_host_wasm.wasm experiments/capability-host/build/authority.wasm
 cp build/capability-host/wasm-target/wasm32-unknown-unknown/release/jadpo_capability_host_guest.wasm experiments/capability-host/build/application.wasm
fi
python3 - <<'PYLOCK'
import hashlib,json,pathlib
base=pathlib.Path('experiments/capability-host/build')
if (base/'authority.wasm').exists():
 (base/'trust.json').write_text(json.dumps({k:hashlib.sha256((base/p).read_bytes()).hexdigest() for k,p in [('authority','authority.wasm'),('contract','contract.json')]},indent=2)+'\n')
PYLOCK
