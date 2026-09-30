#!/usr/bin/env python3
"""Collect raw runner records without changing measurements or artifacts."""
import hashlib,json,statistics
from pathlib import Path
root=Path(__file__).resolve().parent
data=root/'build/measurements'
def load(path):return json.loads(path.read_text())
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
clean=load(data/'clean-build.json')
incremental=load(data/'incremental-build-sanitized.json')
startup=load(data/'fresh-start-sanitized.json')
for row in incremental['runs']:
 row['details']=json.loads((data/f'incremental-build-sanitized-{row["run"]}.stdout').read_text().splitlines()[-1])
for row in startup['runs']:
 row['details']=load(data/f'fresh-start-sanitized-{row["run"]}.stdout')
def summary(rows):
 values=[r['wallSeconds']for r in rows]
 return {'count':len(values),'minimumSeconds':min(values),'medianSeconds':statistics.median(values),'maximumSeconds':max(values)}
expected='8b85d7d65e04f546ab8adb1b25eb654f5c7802c175ea610caaf35e8d5eb71a13'
actual=sha(root/'build/probe.wasm')
assert actual==expected
assert sha(root.parent/'fixture/app.jadpo')==load(root.parent/'acceptance.json')['source']['sha256']
assert len(clean['runs'])==5 and len(incremental['runs'])==5 and len(startup['runs'])==20
assert all(r['exitCode']==0 for group in [clean,incremental,startup] for r in group['runs'])
assert all(r['details']['pass'] and r['details']['artifactSha256']==expected for r in startup['runs'])
assert all(r['details']['finalLinearMemoryPages']==6 for r in startup['runs'])
context=load(data/'context.json')
for relative in ['experiments/wasm-exp1/rust/incremental.py','experiments/wasm-exp1/rust/startup.ts']:
 context['sourceAndToolHashes'][relative]=sha(root.parents[2]/relative)
(data/'context.json').write_text(json.dumps(context,indent=2)+'\n')
report={'schemaVersion':1,'route':'rust','scope':'Probe only; descriptive local observations with concurrent workstation activity; no target/route performance comparison.',
 'context':context,'artifactSha256':actual,'artifactBytes':(root/'build/probe.wasm').stat().st_size,
 'originalSourceSha256':sha(root.parent/'fixture/app.jadpo'),
 'frozenGeneratorSha256':sha(root/'generate.py'),'frozenRuntimeSha256':sha(root/'runtime.rs'),
 'cleanBuild':{'scope':'Projection -> generated Rust -> release Wasm. Remove only Cargo target directory before each run. Cached downloads and already-built checked frontend retained. Cleanup time excluded.','summary':summary(clean['runs']),'raw':clean},
 'incrementalBuild':{'scope':'Alternate source min_length 3->4->3->4->3->4 in private copy; normal check, projection, warm backend build, then semantic startup. Per-stage timings retained. Broader scope than clean backend build.','summary':summary(incremental['runs']),'raw':incremental},
 'freshStart':{'scope':'New Bun process per run with --no-install --env-file=/dev/null; frozen Wasm compile+instantiate+single successful probe through common driver, synthetic host. No artificial delay. External process wall includes startup/imports/exit. Internal phase timings exclude some process overhead.','summary':summary(startup['runs']),'raw':startup},
 'rssInterpretation':'residentSetBytes measures the entire Bun host process after execution, not isolated Wasm usage. runtimeResourceUsage is retained in raw Bun units; no conversion assumed.',
 'memoryInterpretation':'The exact common-driver instance is observed by wrapping WebAssembly.instantiate. All runs end at 6 linear-memory pages; compiled module minimum5 maximum128. This does not measure host or allocator peak beyond process RSS.',
 'supersededBatch':'Initial incremental-build.json remains raw historical evidence but is excluded from summaries because it preceded explicit Bun environment isolation and per-stage instrumentation.',
 'restore':{'originalArtifactHashVerified':True,'originalSourceUnmodified':True}}
(root/'measurements.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({name:report[name]['summary']for name in ['cleanBuild','incrementalBuild','freshStart']},indent=2))
