#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
python3 - <<'PY'
from pathlib import Path
root=Path('experiments/wasm-exp1/rust-full')
source=Path('experiments/wasm-exp1/fixture/app.jadpo').read_text()
assert source.count('min_length: 3')==1
(root/'build/mutated-source').mkdir(parents=True,exist_ok=True)
(root/'build/mutated-source/app.jadpo').write_text(source.replace('min_length: 3','min_length: 5'))
PY
jadpo/target/debug/jadpo check experiments/wasm-exp1/rust-full/build/mutated-source
experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection experiments/wasm-exp1/rust-full/build/mutated-source experiments/wasm-exp1/rust-full/build/mutated-projection
python3 experiments/wasm-exp1/rust-full/generate.py experiments/wasm-exp1/rust-full/build/mutated-projection/program.json --output experiments/wasm-exp1/rust-full/build/mutated
python3 - <<'PY'
from pathlib import Path
root=Path('experiments/wasm-exp1/rust-full')
(root/'build/mutated/Cargo.toml').write_text((root/'Cargo.toml').read_text().replace('path = "build/generated.rs"','path = "generated.rs"'))
(root/'build/mutated/Cargo.lock').write_bytes((root/'Cargo.lock').read_bytes())
PY
export RUSTFLAGS="-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144"
cargo build --offline --locked --manifest-path experiments/wasm-exp1/rust-full/build/mutated/Cargo.toml --target wasm32-unknown-unknown --target-dir experiments/wasm-exp1/rust-full/build/mutated-cargo --release
cp experiments/wasm-exp1/rust-full/build/mutated-cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm experiments/wasm-exp1/rust-full/build/mutated-probe.wasm
bun --no-install --env-file=/dev/null experiments/wasm-exp1/rust-full/mutation.ts
