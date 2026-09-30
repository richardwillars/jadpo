#!/usr/bin/env python3
import hashlib,json,subprocess,time
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
files=['build/probe.wasm','build/generated.rs','build/manifest.json','Cargo.lock']
before={name:digest(HERE/name) for name in files}
source=ROOT/'experiments/wasm-exp1/fixture/app.jadpo'
source_before=digest(source)
commands=[['cargo','clean','--manifest-path','experiments/wasm-exp1/rust-full/Cargo.toml','--target-dir','experiments/wasm-exp1/rust-full/build/cargo'],['sh','experiments/wasm-exp1/rust-full/build.sh']]
stages=[]
for command in commands:
 started=time.perf_counter();r=subprocess.run(command,cwd=ROOT,text=True,capture_output=True)
 stages.append({'command':command,'exitCode':r.returncode,'wallSeconds':time.perf_counter()-started,'stdout':r.stdout,'stderr':r.stderr})
 if r.returncode:break
after={name:digest(HERE/name) for name in files if (HERE/name).exists()}
passed=all(s['exitCode']==0 for s in stages) and before==after and source_before==digest(source)
result={'case':'A18-clean-reconstruction','pass':passed,'cacheScope':'Named Rust target directory removed; registry/toolchain/checked projection caches retained.','before':before,'after':after,'sourceSha256':source_before,'stages':stages}
(HERE/'build/rebuild-result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'pass':passed,'artifactSha256':after.get('build/probe.wasm'),'sourceSha256':source_before}))
if not passed:raise SystemExit(1)
