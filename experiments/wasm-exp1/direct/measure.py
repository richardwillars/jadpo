#!/usr/bin/env python3
"""Bounded local probe measurements; no cleanup outside this route's Cargo output."""
import datetime,hashlib,json,os,platform,resource,shutil,statistics,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];DIRECT=ROOT/'experiments/wasm-exp1/direct';OUT=DIRECT/'build/measurements'
OUT.mkdir(parents=True,exist_ok=True)
EXPECTED='4f91575377e515ed9ccb6835edffcf28a8706903f813faf1cced4f0dea7f28aa'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run_group(label,count,command,clean=False):
 rows=[]
 for n in range(1,count+1):
  if clean:shutil.rmtree(DIRECT/'build/cargo',ignore_errors=True)
  started=time.perf_counter();before=resource.getrusage(resource.RUSAGE_CHILDREN)
  with (OUT/f'{label}-{n}.stdout').open('wb') as out,(OUT/f'{label}-{n}.stderr').open('wb') as err:
   result=subprocess.run(command,cwd=ROOT,stdout=out,stderr=err,check=False,timeout=180)
  after=resource.getrusage(resource.RUSAGE_CHILDREN)
  row=dict(run=n,wallSeconds=time.perf_counter()-started,childUserSeconds=after.ru_utime-before.ru_utime,childSystemSeconds=after.ru_stime-before.ru_stime,exitCode=result.returncode)
  if label!='clean-build':row['details']=json.loads((OUT/f'{label}-{n}.stdout').read_text().splitlines()[-1])
  rows.append(row)
  if result.returncode:raise RuntimeError(f'{label} run{n} failed; inspect retained logs')
 report=dict(label=label,recordedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),command=command,
  cleanup='experiments/wasm-exp1/direct/build/cargo' if clean else None,
  cacheScope='Only named route Cargo output removed before clean samples; cleanup excluded. Registry/download/frontend/OS caches retained.',runs=rows)
 (OUT/f'{label}.json').write_text(json.dumps(report,indent=2)+'\n');return report
started=datetime.datetime.now(datetime.timezone.utc)
context=dict(recordedAt=started.isoformat(),platform=platform.platform(),logicalCpuCount=os.cpu_count(),
 backgroundWork='Shared workstation; Rust full-slice compiler and coordinator integration work concurrently. No idle/thermal/power isolation.',
 sourceAndToolHashes={str(p.relative_to(ROOT)):sha(p) for p in [ROOT/'experiments/wasm-exp1/fixture/app.jadpo',ROOT/'experiments/wasm-exp1/compiler/build/projected/program.json',ROOT/'experiments/wasm-exp1/host/driver.ts',ROOT/'experiments/wasm-exp1/tooling/package-lock.json',*[DIRECT/x for x in ['generate.py','runtime.rs','link.ts','Cargo.toml','Cargo.lock','build.sh','startup.ts','incremental.py','measure.py']]]},binaries={})
for name in ['rustc','cargo','bun','python3']:
 path=Path(shutil.which(name)).resolve()
 context['binaries'][name]=dict(path=str(path),sha256=sha(path),version=subprocess.check_output([name,'--version'],text=True).strip())
context['wabtVersion']=json.loads((ROOT/'experiments/wasm-exp1/tooling/node_modules/wabt/package.json').read_text())['version']
(OUT/'context.json').write_text(json.dumps(context,indent=2)+'\n')
clean=run_group('clean-build',5,['sh','experiments/wasm-exp1/direct/build.sh'],True)
(OUT/'incremental-state.json').unlink(missing_ok=True)
incremental=run_group('incremental-build',5,['python3','experiments/wasm-exp1/direct/incremental.py'])
subprocess.run(['sh','experiments/wasm-exp1/direct/build.sh'],cwd=ROOT,check=True,stdout=(OUT/'restore.stdout').open('wb'),stderr=(OUT/'restore.stderr').open('wb'))
assert sha(DIRECT/'build/probe.wasm')==EXPECTED
startup=run_group('fresh-start',20,['bun','--no-install','--env-file=/dev/null','experiments/wasm-exp1/direct/startup.ts'])
assert all(r['details']['pass'] and r['details']['artifactSha256']==EXPECTED for r in startup['runs'])
assert sha(ROOT/'experiments/wasm-exp1/fixture/app.jadpo')=='447ce2f023b53cecd705999ec280ae9b9d61f7ae0e85184b77c62c9f7aa143f1'
def summary(group):
 values=[r['wallSeconds'] for r in group['runs']]
 return dict(count=len(values),minimumSeconds=min(values),medianSeconds=statistics.median(values),maximumSeconds=max(values))
ended=datetime.datetime.now(datetime.timezone.utc)
report=dict(schemaVersion=1,route='direct',scope='Probe descriptive local measurements under uncontrolled concurrent workstation load; not a route/target performance conclusion.',context=context,
 artifactSha256=EXPECTED,artifactBytes=(DIRECT/'build/probe.wasm').stat().st_size,
 cleanBuild=dict(scope='Checked projection -> WAT generation -> clean generic Rust helper release build -> wabt linking. Only helper Cargo target removed; registry, checked frontend and OS caches retained; cleanup excluded.',summary=summary(clean),raw=clean),
 incrementalBuild=dict(scope='Private source min_length3->4->3->4->3->4; normal check, projection, warm generic helper build and direct WAT linking, semantic startup. Per-stage times; broader than clean backend scope.',summary=summary(incremental),raw=incremental),
 freshStart=dict(scope='Fresh Bun subprocess, Wasm compile+instantiate+one successful probe with in-memory host; exact Rust measurement workload. No artificial delay, SQL or network.',summary=summary(startup),raw=startup),
 rssInterpretation='Whole Bun process resident memory after invocation, not isolated Wasm footprint; resourceUsage kept raw with no assumed unit conversion.',
 memoryInterpretation='Exact shared-driver instance observed via instantiate wrapper; final linear-memory pages recorded, not peak allocator consumption.',
 restore=dict(originalArtifactHashVerified=True,originalSourceUnmodified=True),
 effort=dict(startUtc=started.isoformat(),endUtc=ended.isoformat(),scriptWallMinutes=(ended-started).total_seconds()/60,budgetMinutes=15))
(DIRECT/'measurements.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({key:report[key]['summary'] for key in ['cleanBuild','incrementalBuild','freshStart']},indent=2))
