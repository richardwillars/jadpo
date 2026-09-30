#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
python3 - <<'PY'
from pathlib import Path
source=Path('experiments/wasm-exp1/fixture/app.jadpo').read_text()
variants={
 'renamed':source.replace('Item','Widget').replace('owner_id','holder_id').replace('title','caption').replace('action probe(', 'action inspect_item('),
 'branch':source.replace('success(title) => title','success(title) => input.title'),
}
for name,text in variants.items():
 out=Path('experiments/wasm-exp1/direct/build/variants')/name/'source'
 out.mkdir(parents=True,exist_ok=True);(out/'app.jadpo').write_text(text)
PY
for DIRECT_VARIANT in renamed branch; do
 DIRECT_VARIANT_ROOT="experiments/wasm-exp1/direct/build/variants/$DIRECT_VARIANT"
 jadpo/target/debug/jadpo check "$DIRECT_VARIANT_ROOT/source"
 experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection "$DIRECT_VARIANT_ROOT/source" "$DIRECT_VARIANT_ROOT/projection"
 if [ "$DIRECT_VARIANT" = renamed ]; then DIRECT_VARIANT_ENTRY=inspect_item; else DIRECT_VARIANT_ENTRY=probe; fi
 sh experiments/wasm-exp1/direct/build.sh "$DIRECT_VARIANT_ROOT/projection/program.json" "$DIRECT_VARIANT_ROOT/module" "$DIRECT_VARIANT_ENTRY"
done
bun --no-install --env-file=/dev/null experiments/wasm-exp1/direct/source-variants.ts
