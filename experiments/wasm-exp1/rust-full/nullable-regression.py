#!/usr/bin/env python3
"""Regression for resolved nullable one- and multi-parameter entry boundaries."""
import json,os,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[2]
base=(ROOT/'experiments/wasm-exp1/fixture/app.jadpo').read_text()
cases=[('multiple','probe',base.replace('action probe(input: ProbeInput)','action probe(input: ProbeInput, unused: ItemTitle?)')),
 ('single','nullable_echo',base+'\nfunction nullable_echo(input: ItemTitle?) -> ItemTitle? { return input }\n')]
results=[]
for label,entry,source in cases:
 path=HERE/'build/nullable'/label;(path/'source').mkdir(parents=True,exist_ok=True)
 (path/'source/app.jadpo').write_text(source)
 commands=[['jadpo/target/debug/jadpo','check',str(path/'source')],
 ['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(path/'source'),str(path/'projection')],
 ['python3',str(HERE/'generate.py'),str(path/'projection/program.json'),'--entry',entry,'--output',str(path/'crate')]]
 for command in commands:subprocess.run(command,cwd=ROOT,check=True,capture_output=True)
 (path/'crate/Cargo.toml').write_text((HERE/'Cargo.toml').read_text().replace('path = "build/generated.rs"','path = "generated.rs"'))
 (path/'crate/Cargo.lock').write_bytes((HERE/'Cargo.lock').read_bytes())
 env={**os.environ,'RUSTFLAGS':'-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144'}
 build=['cargo','build','--offline','--locked','--manifest-path',str(path/'crate/Cargo.toml'),'--target','wasm32-unknown-unknown','--target-dir',str(HERE/'build/nullable/cargo'),'--release']
 built=subprocess.run(build,cwd=ROOT,env=env,text=True,capture_output=True);(path/'build.log').write_text(built.stdout+built.stderr);built.check_returncode()
 module=HERE/'build/nullable/cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm'
 (path/'module.wasm').write_bytes(module.read_bytes())
 run=subprocess.run(['bun','--no-install','--env-file=/dev/null',str(HERE/'nullable-regression.ts'),str(path)],cwd=ROOT,text=True,capture_output=True)
 (path/'result.json').write_text(run.stdout);run.check_returncode();results.append(json.loads(run.stdout))
(HERE/'build/nullable/results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results,indent=2))
