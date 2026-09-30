#!/usr/bin/env python3
"""Prepared matched-scope build measurement. Do not run during throughput work."""
import argparse,hashlib,json,os,shutil,subprocess,time
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[2]
parser=argparse.ArgumentParser();parser.add_argument('--route',choices=['rust','bun'],required=True);parser.add_argument('--mode',choices=['clean','incremental'],required=True);a=parser.parse_args()
work=HERE/'build/matched-measurement'/a.route;work.mkdir(parents=True,exist_ok=True)
state=work/f'state-{a.mode}.json';iteration=(json.loads(state.read_text())['iteration'] if state.exists() else 0)+1
minimum=3 if a.mode=='clean' else (4 if iteration%2 else 3)
source=work/'source';source.mkdir(exist_ok=True)
original=(ROOT/'experiments/wasm-exp1/fixture/app.jadpo').read_text();assert original.count('min_length: 3')==1
(source/'app.jadpo').write_text(original.replace('min_length: 3',f'min_length: {minimum}'))
projection=work/'projected';crate=work/'crate';target=work/'cargo'
# Cleanup is outside the measured stages and restricted to this private subtree.
if a.mode=='clean':
 for path in [projection,crate,target]:shutil.rmtree(path,ignore_errors=True)
steps=[]
def run(stage,command,env=None):
 started=time.perf_counter();r=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True);elapsed=time.perf_counter()-started
 (work/f'{a.mode}-{iteration}-{stage}.stdout').write_text(r.stdout);(work/f'{a.mode}-{iteration}-{stage}.stderr').write_text(r.stderr)
 steps.append({'stage':stage,'command':command,'wallSeconds':elapsed,'exitCode':r.returncode});r.check_returncode();return r
start=time.perf_counter()
run('check',['jadpo/target/debug/jadpo','check',str(source)])
project=['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(source),str(projection)]
if a.route=='bun':project.append('--bun')
run('projection' if a.route=='rust' else 'projectionAndBunBackend',project)
if a.route=='rust':
 run('rustGeneration',['python3',str(HERE/'generate.py'),str(projection/'program.json'),'--output',str(crate)])
 (crate/'Cargo.toml').write_text((HERE/'Cargo.toml').read_text().replace('path = "build/generated.rs"','path = "generated.rs"'))
 (crate/'Cargo.lock').write_bytes((HERE/'Cargo.lock').read_bytes())
 env={**os.environ,'RUSTFLAGS':'-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144'}
 run('rustBackend',['cargo','build','--offline','--locked','--manifest-path',str(crate/'Cargo.toml'),'--target','wasm32-unknown-unknown','--target-dir',str(target),'--release'],env)
 artifact=target/'wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm'
else:artifact=projection
smoke=run('semanticStartup',['bun','--no-install','--env-file=/dev/null',str(HERE/'startup.ts'),a.route,str(artifact),str(projection/'program.json')])
semantic=json.loads(smoke.stdout);assert semantic['pass']
result={'route':a.route,'mode':a.mode,'iteration':iteration,'minimumLength':minimum,'sourceSha256':hashlib.sha256((source/'app.jadpo').read_bytes()).hexdigest(),
 'projectionSha256':hashlib.sha256((projection/'program.json').read_bytes()).hexdigest(),'measuredStagesSeconds':time.perf_counter()-start,'steps':steps,'semantic':semantic,
 'scope':'Normal source check + checked projection + target emission/build + real SQLite successful probe. Bun projection/backend are one compiler invocation; Rust emits/compiles from projection separately.',
 'cacheScope':'Clean removes only this route private generated/projected/Cargo target dirs. Installed frontend, registry downloads, toolchain and OS caches retained. Incremental alternates min3/4 with warm target.'}
state.write_text(json.dumps({'iteration':iteration})+'\n');(work/f'{a.mode}-{iteration}.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
