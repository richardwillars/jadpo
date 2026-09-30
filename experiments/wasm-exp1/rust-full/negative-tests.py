#!/usr/bin/env python3
"""Actual source -> normal check -> projection -> bounded lowering rejections."""
import hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
BASE=(ROOT/'experiments/wasm-exp1/fixture/app.jadpo').read_text()
OUT=HERE/'build/negative'
OUT.mkdir(parents=True,exist_ok=True)
handled=BASE.replace('return attempt Item.rename(second_id, second_title)', '''return match Item.rename(second_id, second_title) {
        success(item) => item
        failure ItemMissing => attempt Item.read(first_id)
        failure ItemConflict => attempt Item.read(first_id)
    }''')
sources={
 'reachable-if':BASE.replace('action probe(input: ProbeInput) -> ItemTitle {','action probe(input: ProbeInput) -> ItemTitle {\n    if true { return input.title }'),
 'handled-nested-mutation':handled,
 'unavailable-freshness':BASE.replace('freshness: authoritative','freshness: eventual'),
 'atomic-cross-store':(ROOT/'tests/compile/fail/120_atomic_domain_mismatch.jadpo').read_text(),
}
results=[]
for name,source in sources.items():
 path=OUT/name;path.mkdir(exist_ok=True);(path/'app.jadpo').write_text(source)
 commands=[['jadpo/target/debug/jadpo','check',str(path)],
   ['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(path),str(path/'projection')],
   ['python3',str(HERE/'generate.py'),str(path/'projection/program.json'),'--output',str(path/'artifact')]]
 stages=[]
 for stage,command in zip(['normal-check','checked-projection','rust-lowering'],commands):
  result=subprocess.run(command,cwd=ROOT,text=True,capture_output=True)
  stages.append({'stage':stage,'command':command,'exitCode':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
  if result.returncode:break
 passed=bool(stages[-1]['exitCode']) and not (path/'artifact/generated.rs').exists()
 results.append({'case':name,'pass':passed,'sourceSha256':hashlib.sha256(source.encode()).hexdigest(),'stages':stages,'artifactWritten':(path/'artifact/generated.rs').exists()})
report={'scope':'A17 source/checked-lowering negative cases; host-import rejection tested separately','results':results}
(OUT/'results.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps([{'case':r['case'],'pass':r['pass'],'rejectedAt':r['stages'][-1]['stage']}for r in results],indent=2))
if not all(r['pass']for r in results):raise SystemExit(1)
