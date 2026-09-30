#!/usr/bin/env python3
"""One measured source edit/check/project/generate/build iteration."""
import hashlib,json,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
RUST=ROOT/'experiments/wasm-exp1/rust'
state=RUST/'build/measurements/incremental-state.json'
prior=json.loads(state.read_text())['iteration'] if state.exists() else 0
iteration=prior+1
minimum=4 if iteration%2 else 3
original=(ROOT/'experiments/wasm-exp1/fixture/app.jadpo').read_text()
assert original.count('min_length: 3')==1
source=RUST/'build/incremental-source'
source.mkdir(parents=True,exist_ok=True)
(source/'app.jadpo').write_text(original.replace('min_length: 3',f'min_length: {minimum}'))
projection=RUST/'build/incremental-projection'
commands=[
 ['jadpo/target/debug/jadpo','check',str(source.relative_to(ROOT))],
 ['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(source.relative_to(ROOT)),str(projection.relative_to(ROOT))],
 ['sh','experiments/wasm-exp1/rust/build.sh',str((projection/'program.json').relative_to(ROOT))],
 ['bun','--no-install','--env-file=/dev/null','experiments/wasm-exp1/rust/startup.ts'],
]
steps=[]
for label,command in zip(['check','projection','backend','semanticStartupCheck'],commands):
 started=time.perf_counter()
 subprocess.run(command,cwd=ROOT,check=True)
 steps.append({'stage':label,'command':command,'wallSeconds':time.perf_counter()-started})
result={'iteration':iteration,'minimumLength':minimum,
 'sourceSha256':hashlib.sha256((source/'app.jadpo').read_bytes()).hexdigest(),
 'projectionSha256':hashlib.sha256((projection/'program.json').read_bytes()).hexdigest(),
 'artifactSha256':hashlib.sha256((RUST/'build/probe.wasm').read_bytes()).hexdigest(),
 'semanticCheck':'frozen valid input returns alpha exactly once','commands':commands,'steps':steps}
state.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
