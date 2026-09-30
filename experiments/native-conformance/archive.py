#!/usr/bin/env python3
"""Retain the conformance artifacts and verify all frozen controls, without DBs."""
import pathlib,json,hashlib,gzip,re
root=pathlib.Path.cwd();base=root/'experiments/native-conformance';out=root/'build/native-conformance';evidence=base/'evidence';records=[]
def sha(b):return hashlib.sha256(b).hexdigest()
def save(path,name):
 b=path.read_bytes();dest=evidence/(name+'.gz');dest.write_bytes(gzip.compress(b,mtime=0));records.append({'artifact':dest.name,'source':str(path.relative_to(root)),'sha256':sha(b),'bytes':len(b),'archiveSha256':sha(dest.read_bytes())})
frozen=json.loads((evidence/'freeze.json').read_text());checks={p:sha((root/p).read_bytes())==h for p,h in frozen.items()};assert all(checks.values()),[p for p,ok in checks.items() if not ok]
for name in ['correctness','boundaries','concurrency']:
 for ext in ['json','log']:save(out/f'{name}.{ext}',f'{name}.{ext}')
for name in ['rust-tests.log','wasm-tests.log','build.log','mutation.log','clean-wasm.log','rebuild.json']:save(out/name,name)
for p in sorted((out/'mutation').rglob('*')):
 if p.is_file() and p.suffix in ['.rs','.wasm','.json','.log','.jadpo']:save(p,'mutation-'+str(p.relative_to(out/'mutation')).replace('/','-'))
for p in sorted(base.rglob('*')):
 if p.is_file() and 'evidence' not in p.relative_to(base).parts and '__pycache__' not in p.parts and p.suffix in ['.rs','.toml','.lock','.py','.mjs','.ts','.sh','.json','.wasm']:
  save(p,'source-'+str(p.relative_to(base)).replace('/','-'))
save(out/'target/release/jadpo-native-conformance','native-executable')
correct=json.loads((out/'correctness.json').read_text());boundary=json.loads((out/'boundaries.json').read_text());concurrency=json.loads((out/'concurrency.json').read_text());mutation=json.loads((out/'mutation/results.json').read_text());rebuild=json.loads((out/'rebuild.json').read_text())
assert len(correct)==6 and len(boundary)==3 and len(concurrency)==3
assert all(c['rolledBack'] and not c['targetDifference'] for server in boundary for c in server['cases'] if c['name']=='oversized write result')
assert all(c['snapshotVerified'] for c in concurrency)
assert sha((base/'build/application.wasm').read_bytes())==next(iter(rebuild['wasmHashes'].values()))
summary={'scope':'local conformance, no fresh performance claim; new WASM is Bun-hosted','httpCases':sum(len(s['cases']) for s in correct),'httpServers':len(correct),'boundaryObservations':sum(len(s['cases']) for s in boundary),'concurrentCalls':sum(s['verified'] for s in concurrency),'rustTestsPassed':sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(out/'rust-tests.log').read_text()))),'wasmTestsPassed':int(re.search(r'(\d+) pass\n',(out/'wasm-tests.log').read_text())[1]),'wasmAssertions':int(re.search(r'(\d+) expect\(\)',(out/'wasm-tests.log').read_text())[1]),'mutation':mutation,'cleanWasmIdentical':rebuild['cleanWasmIdentical'],'generatedRustSha256':sha((base/'build/application.rs').read_bytes()),'wasmSha256':sha((base/'build/application.wasm').read_bytes()),'nativeSha256':sha((out/'target/release/jadpo-native-conformance').read_bytes())}
(base/'results.json').write_text(json.dumps(summary,indent=2)+'\n');(evidence/'manifest.json').write_text(json.dumps({'freezeChecks':checks,'results':summary,'artifacts':records},indent=2)+'\n');print(json.dumps(summary))
