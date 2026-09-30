"""Archive raw observations, exact candidates and new experiment sources."""
import pathlib,json,hashlib,gzip,subprocess
root=pathlib.Path('.');base=root/'experiments/auth-policy-measure';out=root/'build/auth-policy-measure';evidence=base/'evidence';evidence.mkdir(exist_ok=True)
subprocess.run(['python3',str(base/'freeze.py')],check=True)
http=json.loads((out/'http.json').read_text());startup=json.loads((out/'startup.json').read_text())
assert 'failure' not in http and 'failure' not in startup
paths=[out/'http.json',out/'http.log',out/'startup.json',out/'startup.log',out/'smoke.json',out/'trust.log',out/'build-times/results.json',root/'build/auth-policy/seed.json',root/'build/auth-policy/seed.sqlite',root/'experiments/auth-policy/build/application.wasm',root/'experiments/auth-policy/build/application.rs',root/'experiments/auth-policy/build/contract.json',root/'build/auth-policy/target/release/jadpo-auth-policy-native']
paths+=sorted((out/'build-times').glob('*.log'))
paths+=sorted(p for p in base.iterdir() if p.is_file())
paths+=[root/'docs/auth-policy-performance-results.md',root/'docs/wasm-host-trust-decision.md',root/'README.md']
for r in http['results']+startup['results']:
 d=root/r['evidenceDir'];paths.append(d/'stderr.log')
 if r.get('count'):paths.append(d/'latencies.json.gz')
manifest={'scope':'Frozen auth-policy artifacts; local Bun-hosted WASM; all credentials synthetic','files':[]}
for index,path in enumerate(paths):
 data=path.read_bytes();compressed=gzip.compress(data,mtime=0);name=f'{index:04d}-{path.name}.gz'
 (evidence/name).write_bytes(compressed)
 manifest['files'].append({'source':str(path),'archive':name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'compressedSha256':hashlib.sha256(compressed).hexdigest()})
(evidence/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
for item in manifest['files']:
 blob=(evidence/item['archive']).read_bytes()
 assert hashlib.sha256(blob).hexdigest()==item['compressedSha256']
 assert hashlib.sha256(gzip.decompress(blob)).hexdigest()==item['sha256']
print(f"Verified {len(paths)} archived observations/artifacts/source files")
