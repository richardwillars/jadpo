#!/usr/bin/env python3
"""Archive one finished, frozen campaign, before changing measured harnesses."""
import pathlib,json,hashlib,gzip,tarfile,io
root=pathlib.Path.cwd();base=root/'experiments/native-exp1';ev=base/'evidence';raw=root/'build/native-exp1';records=[]
def digest(b):return hashlib.sha256(b).hexdigest()
def save(path,name):
 b=path.read_bytes();dest=ev/(name+'.gz');dest.write_bytes(gzip.compress(b,mtime=0));records.append({'artifact':dest.name,'source':str(path.relative_to(root)),'sha256':digest(b),'bytes':len(b),'archiveSha256':digest(dest.read_bytes())})
measured=json.loads((raw/'http.json').read_text())
for p,h in measured['hashes'].items():
 path=raw/'target/release/jadpo-native-exp1' if p=='executable' else base/p
 assert digest(path.read_bytes())==h,(p,'measured source changed')
for name in ['http','delete','micro','startup','smoke','correctness','boundaries']:
 for ext in ['json','log']:
  p=raw/(name+'.'+ext)
  if p.exists():save(p,'raw-'+p.name)
for p in [raw/'rust-tests.log',raw/'mutation.log',raw/'build.log',raw/'build-times.log']:
 save(p,p.name)
for p in sorted((raw/'mutation').rglob('*')):
 if p.is_file() and (p.suffix in ['.log','.json','.jadpo'] or p.name.endswith('.log')):save(p,'mutation-'+str(p.relative_to(raw/'mutation')).replace('/','-'))
for p in sorted((raw/'build-times').glob('*.log')):save(p,'build-time-'+p.name)
save(raw/'build-times/results.json','build-times.json')
for p in ['src/main.rs','src/shared.rs','src/storage.rs','prepare.py','Cargo.toml','Cargo.lock','bun-server.ts','harness.mjs','measure.mjs','correctness.mjs','boundaries.mjs','build/generated.rs','build/storage.json','build/manifest.json','build/row-codec.json']:
 save(base/p,'measured-'+p.replace('/','-'))
save(root/'experiments/wasm-exp1/compiler/build/projected/program.json','checked-projection.json')
save(root/'experiments/wasm-exp1/fixture/app.jadpo','fixture.jadpo')
save(raw/'target/release/jadpo-native-exp1','native-measured')
save(next((raw/'build-times').glob('clean-*/release/jadpo-native-exp1')),'native-clean')
# Archive every generated Bun text prerequisite, not test databases.
files={str(p.relative_to(root/'build/wasm-exp1/baseline/generated')):p.read_bytes() for p in (root/'build/wasm-exp1/baseline/generated/bun').rglob('*') if p.is_file() and p.suffix in ['.ts','.json','.sql']}
stream=io.BytesIO()
with tarfile.open(fileobj=stream,mode='w') as tar:
 for name,b in sorted(files.items()):
  info=tarfile.TarInfo(name);info.size=len(b);info.mtime=0;tar.addfile(info,io.BytesIO(b))
archive=ev/'bun-generated.tar.gz';archive.write_bytes(gzip.compress(stream.getvalue(),mtime=0));records.append({'artifact':archive.name,'archiveSha256':digest(archive.read_bytes()),'files':{k:digest(v) for k,v in files.items()}})
frozen=json.loads((ev/'freeze.json').read_text());checks={p:digest((root/p).read_bytes())==h for p,h in frozen['files'].items()};assert all(checks.values())
(ev/'manifest.json').write_text(json.dumps({'measuredNativeSha256':measured['hashes']['executable'],'freezeChecks':checks,'artifacts':records,'notes':['No databases or unrelated edited file contents are archived.','Initial peakRssBytes in raw Bun cells has a 1024x unit error; summary corrects it. ps RSS samples are unaffected.','Measured source snapshots are authoritative for these campaigns; later telemetry-only fixes do not change those bytes.']},indent=2)+'\n')
print(json.dumps({'archives':len(records),'freezeChecks':checks}))
