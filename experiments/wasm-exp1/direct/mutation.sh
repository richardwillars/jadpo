#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
python3 - <<'PY'
from pathlib import Path
source=Path('experiments/wasm-exp1/fixture/app.jadpo').read_text()
assert source.count('min_length: 3')==1
out=Path('experiments/wasm-exp1/direct/build/mutated-source')
out.mkdir(parents=True,exist_ok=True)
(out/'app.jadpo').write_text(source.replace('min_length: 3','min_length: 5'))
PY
jadpo/target/debug/jadpo check experiments/wasm-exp1/direct/build/mutated-source
experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection experiments/wasm-exp1/direct/build/mutated-source experiments/wasm-exp1/direct/build/mutated-projection
sh experiments/wasm-exp1/direct/build.sh experiments/wasm-exp1/direct/build/mutated-projection/program.json experiments/wasm-exp1/direct/build/mutated
cp experiments/wasm-exp1/direct/build/mutated/probe.wasm experiments/wasm-exp1/direct/build/mutated-probe.wasm
