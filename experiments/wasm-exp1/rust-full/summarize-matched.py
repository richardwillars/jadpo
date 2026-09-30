#!/usr/bin/env python3
import hashlib,json,statistics
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[2];RAW=HERE/'build/full-measurements'
def load(p):return json.loads(p.read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def stats(values):return {'count':len(values),'minimum':min(values),'median':statistics.median(values),'maximum':max(values)}
groups={}
for route in ['rust','bun']:
 for kind,count in [('clean',5),('incremental',5),('first-request',20)]:
  label=f'full-{route}-{kind}';record=load(RAW/f'{label}.json');assert len(record['runs'])==count
  for r in record['runs']:
   assert r['exitCode']==0
   r['detail']=load(RAW/f'{label}-{r["run"]}.stdout')
   semantic=r['detail'] if kind=='first-request' else r['detail']['semantic']
   assert semantic['pass'] and semantic['output']=={'kind':'success','value':'alpha'}
  summary={'processWallSeconds':stats([r['wallSeconds']for r in record['runs']])}
  if kind!='first-request':
   summary['measuredPipelineSeconds']=stats([r['detail']['measuredStagesSeconds']for r in record['runs']])
   names=[s['stage']for s in record['runs'][0]['detail']['steps']]
   summary['stageWallSeconds']={n:stats([next(s['wallSeconds']for s in r['detail']['steps']if s['stage']==n)for r in record['runs']])for n in names}
  else:
   summary['scriptTotalMs']=stats([r['detail']['scriptTotalMs']for r in record['runs']])
   summary['residentSetBytes']=stats([r['detail']['residentSetBytes']for r in record['runs']])
   if route=='rust':summary['finalLinearMemoryPages']=sorted({r['detail']['phases']['finalLinearMemoryPages']for r in record['runs']})
  groups[label]={'summary':summary,'raw':record}
frozen=sha(HERE/'build/probe.wasm');assert frozen=='bf23ef07059ab0a469409cb980d5f8aade0dd3160ee09157e062f1e8d5cfb6ad'
source=sha(HERE.parent/'fixture/app.jadpo');assert source==load(HERE.parent/'acceptance.json')['source']['sha256']
assert all(r['detail']['artifactSha256']==frozen for r in groups['full-rust-first-request']['raw']['runs'])
clean_hashes={r['detail']['semantic']['artifactSha256']for r in groups['full-rust-clean']['raw']['runs']};assert len(clean_hashes)==1
generated=load(HERE/'build/manifest.json')['generatedRustSha256']
assert all(load(HERE/f'build/matched-measurement/rust/clean-{i}-rustGeneration.stdout')['generatedRustSha256']==generated for i in range(1,6))
report={'schemaVersion':1,'scope':'Descriptive local matched logical build pipelines and first-request startup, no causal target ranking. No duplicate readiness-only samples.',
 'context':load(RAW/'context.json'),'groups':groups,
 'frozenArtifacts':{'wasmSha256':frozen,'sourceSha256':source,'bunAppSha256':sha(ROOT/'build/wasm-exp1/baseline/generated/bun/target/app.ts'),'bunProjectionSha256':sha(ROOT/'build/wasm-exp1/baseline/generated/program.json')},
 'buildLayoutQualifier':{'privateCleanWasmSha256':next(iter(clean_hashes)),'frozenGeneratedRustSha256':generated,'allFiveCleanGeneratedRustSourcesMatchFrozen':True,'observation':'Private crate uses generated.rs as lib path; frozen crate uses build/generated.rs. Those different source-path strings are observable in Wasm panic data and artifact hashes differ. Same-layout clean builds repeat the same hash. First-request measurements use frozen deployed Wasm bytes, not the private build or final incremental variant.'},
 'qualifications':['Clean measured stages exclude private directory cleanup; external process wall includes wrapper and cleanup overhead.','Installed frontend/toolchain/registry download cache/OS caches retained. No network installation.','Bun checked projection and backend emission are combined; Rust emits then compiles Rust. Both pipelines include normal check and real SQLite semantic smoke.','Fresh-process first-request includes schema/seed setup, generated runtime import/compilation, trusted owner policy and SQLite read. This differs from earlier readiness-only and synthetic-host measurements.','RSS is the entire Bun host process. Raw resourceUsage values retained without unit assumptions.','Sequential measured jobs after the coordinator performance slot opened; no thermal/power/OS-cache isolation claim.']}
(HERE/'matched-measurements.json').write_text(json.dumps(report,indent=2)+'\n')
table=['# Matched build and first-request observations','','All 20 build samples and 40 first-request samples returned the expected result. Raw data, stage timings, source/tool hashes and exact commands are in `matched-measurements.json`; unedited logs remain under `build/full-measurements/`.','','| Route | Operation | Samples | Median process wall | Range |','| --- | --- | ---: | ---: | ---: |']
for route in ['rust','bun']:
 for kind in ['clean','incremental','first-request']:
  x=groups[f'full-{route}-{kind}']['summary']['processWallSeconds'];table.append(f'| {route} | {kind} | {x["count"]} | {x["median"]*1000:.3f} ms | {x["minimum"]*1000:.3f}–{x["maximum"]*1000:.3f} ms |')
table+=['','The build pipelines have the same logical scope: normal source check, checked projection, target emission/build and a verified real-SQLite probe. Bun projection/backend emission is one compiler step; Rust generation and compilation are separate. Process wall also includes private cleanup and wrapper overhead. See the JSON stage timings for narrower attribution.','','Clean builds remove private generated/projected/Cargo target directories. Installed compiler/toolchain, package downloads and OS caches remain. Incremental runs alternate a private source constraint between three and four; the frozen source and deployed artifacts are untouched.','','Private Rust clean build artifacts have a different hash from the deployed artifact because the Cargo lib source path differs (`generated.rs` versus `build/generated.rs`); panic source-path strings differ. All five generated Rust sources exactly match the frozen source hash, and all five private clean modules have the same hash. The first-request samples use the original frozen module and original Bun baseline, not private or incremental variants.','','First-request samples include a new process, module import/compilation, SQLite schema/seed setup and one owner-scoped read. They are not duplicate readiness-only measurements. Rust instances finish at six linear-memory pages. RSS includes Bun and host glue, not just Wasm.','','The coordinator confirmed the performance slot was free; measured workloads ran sequentially. Thermal state, power settings and OS caches were not isolated. These observations are not a causal speedup or long-term target recommendation. Reproduction commands are in `measurement-preparation.md`; the optional Bun first-request command is recorded verbatim in the JSON.']
(HERE/'matched-measurements.md').write_text('\n'.join(table)+'\n')
print(json.dumps({k:v['summary']for k,v in groups.items()},indent=2))
