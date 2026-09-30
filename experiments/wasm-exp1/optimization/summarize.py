from pathlib import Path
import json,hashlib,statistics,gzip
r=Path(__file__).resolve().parents[3];o=r/'experiments/wasm-exp1/optimization';b=r/'build/wasm-exp1/optimization'
d=json.loads((b/'comparison/results.json').read_text());assert len(d['results'])==80 and all(x['errors']==0 for x in d['results']);table=[]
for concurrency in [1,8]:
 for name in ['bun','fresh_original','fresh_reset','pooled','pooled_prepared','pooled_cached','fresh_mock','pooled_mock']:
  xs=[x for x in d['results'] if x['target']==name and x['concurrency']==concurrency];assert len(xs)==5 and sorted(x['run'] for x in xs)==[1,2,3,4,5]
  row={'target':name,'concurrency':concurrency,'requests':sum(x['requests'] for x in xs)}
  for key in ['successfulThroughput','p50Ms','p95Ms','p99Ms']:
   vals=[x[key] for x in xs];row[key]={'median':statistics.median(vals),'min':min(vals),'max':max(vals)}
  table.append(row)
d['summary']=table;d['interpretation']=['Local synchronous SQLite operation throughput, not HTTP capacity.','Five1s warmup+3s timed runs pervariant/concurrency; changed durations belong to this extension.','All timed/warmup results checked; zeroerrors.','All variants run sequentially with rotatedorder. Uncontrolled workstation and short local validation/package jobs overlap early runs.','No pooled-request validation or policy removal; coreJSON/host checks retained.','Mock variants use the same synchronous driver with noDB, not the original asynchronous CPU workload.','RSS includes sharedprocess and samplearrays, not isolated runtime memory.','Plan caching covers immutable checkedmetadata only; Cloudflare still uses its original SQL adapter.']
(o/'comparison.json').write_text(json.dumps(d,indent=2)+'\n')
lines=['| Variant | Concurrency | Operations/s median | p50 ms | p95 ms median (range) | p99 ms |','| --- | ---: | ---: | ---: | ---: | ---: |']
for x in table:
 t=x['successfulThroughput']['median'];p=x['p95Ms'];lines.append(f"| {x['target']} | {x['concurrency']} | {t:.0f} | {x['p50Ms']['median']:.6f} | {p['median']:.6f} ({p['min']:.6f}–{p['max']:.6f}) | {x['p99Ms']['median']:.6f} |")
(o/'table.md').write_text('\n'.join(lines)+'\n');print('\n'.join(lines))
# Retain raw acceptance and repeat aggregates without generated databases or tools.
raw=o/'evidence';raw.mkdir(exist_ok=True);manifest=[]
files={'attribution.json':b/'attribution.json','comparison-raw.json':b/'comparison/results.json','atomic.json':b/'atomic.json','negatives.json':b/'negatives.json','mutation.json':b/'mutation/evidence.json','tests.log':b/'all-tests.log'}
for label,log in [('pool-local.json','correctness.log'),('cached-local.json','cached-correctness.log')]:files[label]=Path(json.loads((b/log).read_text())['evidence'])
for label in ['results.json','tail.jsonl','headers.txt']:
 f=b/'cloud'/label
 if f.exists():files['cloud-'+label]=f
for label,path in files.items():
 data=path.read_bytes();target=raw/(label+'.gz');target.write_bytes(gzip.compress(data,mtime=0));manifest.append({'file':str(target.relative_to(o)),'source':str(path.relative_to(r)),'rawBytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
(o/'evidence/manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
