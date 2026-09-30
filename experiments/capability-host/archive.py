"""One deterministic archive plus per-file hashes for reviewable local evidence."""
import pathlib,subprocess,json,hashlib,tarfile,gzip,io
base=pathlib.Path('experiments/capability-host');out=pathlib.Path('build/capability-host');dest=base/'evidence';dest.mkdir(exist_ok=True)
for script,args in [('freeze.py',[]),('pin.py',['--check'])]:subprocess.run(['python3',str(base/script),*args],check=True)
http=json.loads((out/'performance/http.json').read_text());startup=json.loads((out/'performance/startup.json').read_text());conformance=json.loads((out/'results.json').read_text())
assert len(http['results'])==336 and len(startup['results'])==40 and 'failure' not in http and 'failure' not in startup
assert len(conformance)==6 and all(len(r['records'])==53 for r in conformance)
paths=set(pathlib.Path(p) for p in json.loads((base/'candidate.json').read_text()))
paths.update(p for p in base.rglob('*') if p.is_file() and 'evidence' not in p.parts and '__pycache__' not in p.parts and 'build' not in p.parts)
paths.update([out/p for p in ['results.json','results-variant.json','seed.json','seed.sqlite','authority-tests.log','native-tests.log','cli-tests.log','cli-integration-tests.log','build.log','attack-build.log','instantiation.json']])
paths.update([pathlib.Path('experiments/auth-policy/build/application.wasm'),pathlib.Path('experiments/auth-policy/bun-server.ts'),pathlib.Path('experiments/auth-policy/sqlite-host.ts'),pathlib.Path('experiments/auth-policy/wasm-driver.ts'),pathlib.Path('jadpo/crates/cli/src/experimental_build.rs'),pathlib.Path('docs/capability-host-results.md'),pathlib.Path('README.md')])
paths.update(p for p in (out/'mutation').rglob('*') if p.is_file() and p.suffix in ['.log','.json','.wasm','.rs','.jadpo'])
paths.update(p for p in (out/'build-cost').iterdir() if p.is_file())
paths.update(p for p in (out/'build-isolation').iterdir() if p.is_file())
paths.update([out/'performance'/p for p in ['http.json','startup.json','smoke.json']])
for row in http['results']+startup['results']:
 directory=pathlib.Path(row['evidenceDir']);paths.add(directory/'stderr.log')
 if row.get('count'):
  path=directory/'latencies.json.gz';paths.add(path)
  samples=json.loads(gzip.decompress(path.read_bytes()));assert len(samples)==row['count']
  samples.sort()
  import math
  for key,q in [('p50Ms',.5),('p95Ms',.95),('p99Ms',.99)]:assert samples[math.ceil(len(samples)*q)-1]==row[key]
  assert samples[-1]==row['maxMs'] and row['activeMs']>=1000 and row['errors']==0 and row['snapshotVerified']
  assert row['statements']==row['count']*row['statementsPerRequest']
manifest={'scope':'Local scoped authority and isolated application WASM; all credentials synthetic; Bun hosting only','files':[]}
archive=dest/'observations.tar.gz'
with archive.open('wb') as file:
 with gzip.GzipFile(fileobj=file,mode='wb',mtime=0,filename='') as stream:
  with tarfile.open(fileobj=stream,mode='w|') as tar:
   for path in sorted(paths):
    data=path.read_bytes();name=str(path);item=tarfile.TarInfo(name);item.size=len(data);item.mtime=0;item.mode=0o644
    tar.addfile(item,io.BytesIO(data));manifest['files'].append({'path':name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
manifest['archive']={'file':archive.name,'bytes':archive.stat().st_size,'sha256':hashlib.sha256(archive.read_bytes()).hexdigest()}
(dest/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
with tarfile.open(archive) as tar:
 for record in manifest['files']:
  data=tar.extractfile(record['path']).read();assert hashlib.sha256(data).hexdigest()==record['sha256']
print(f"Verified {len(paths)} archived files and all 336 latency distributions")
