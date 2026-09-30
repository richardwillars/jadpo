from pathlib import Path
import json,hashlib,gzip,statistics,platform
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent;raw=root/'build/wasm-exp1/value-path'
http=json.loads((raw/'http/results.json').read_text());micro=json.loads((raw/'micro/results.json').read_text());diagnostic=json.loads((raw/'write-diagnostic/results.json').read_text())
assert len(diagnostic['results'])==12 and all(r['errors']==0 and r['snapshotVerified'] for r in diagnostic['results'])
assert len(http['results'])==140 and len(micro['results'])==50
assert all(x['errors']==0 and x['snapshotVerified'] for x in http['results'])
assert all(x['errors']==0 for x in micro['results'])
median=statistics.median
summaries=[]
for concurrency in [1,16]:
 for workload in ['read_small','read_large','write_single','write_pair']:
  for target in ['bun','previous','candidate','noop']:
   rows=[x for x in http['results'] if x['concurrency']==concurrency and x['workload']==workload and x['target']==target]
   if not rows:continue
   assert len(rows)==5
   baseline=[x for x in http['results'] if x['concurrency']==concurrency and x['workload']==workload and x['target']=='bun']
   ratios=[r['p95Ms']/next(b['p95Ms'] for b in baseline if b['run']==r['run']) for r in rows]
   summaries.append({'concurrency':concurrency,'workload':workload,'target':target,**{key:median(r[key] for r in rows) for key in ['throughput','p50Ms','p95Ms','p99Ms']},'throughputRange':[min(r['throughput'] for r in rows),max(r['throughput'] for r in rows)],'p95RangeMs':[min(r['p95Ms'] for r in rows),max(r['p95Ms'] for r in rows)],'serverCpuUsPerRequest':median((r['server']['cpu']['user']+r['server']['cpu']['system'])/r['count'] for r in rows),'clientCpuUsPerRequest':median((r['clientCpu']['user']+r['clientCpu']['system'])/r['count'] for r in rows),'handlerP95Ms':median(r['server']['handlerP95Ms'] for r in rows),'pairedP95RatiosToBun':ratios,'pairedThroughputRatiosToBun':[r['throughput']/next(b['throughput'] for b in baseline if b['run']==r['run']) for r in rows],'pairedServerCpuRatiosToBun':[((r['server']['cpu']['user']+r['server']['cpu']['system'])/r['count'])/next((b['server']['cpu']['user']+b['server']['cpu']['system'])/b['count'] for b in baseline if b['run']==r['run']) for r in rows]})
report={'scope':http['protocol'],'httpRequests':sum(r['count'] for r in http['results']),'microOperations':sum(r['requests'] for r in micro['results']),'errors':0,'writeDiagnosticRequests':sum(r['count'] for r in diagnostic['results']),'http':summaries,'micro':[{ 'size':size,'target':target,**{key:median(r[key] for r in micro['results'] if r['size']==size and r['target']==target) for key in ['throughput','p50Ms','p95Ms','p99Ms']}} for size in [256,16384] for target in ['bun','previous','speed','ownership','combined']]}
(out/'results.json').write_text(json.dumps(report,indent=2)+'\n')
lines=['# Local HTTP results','', 'Medians of five runs; times are end-to-end milliseconds. CPU is total server user+system microseconds per completed request. See raw runs for variation.','', '| Concurrency | Workload | Target | Requests/s | p50 ms | p95 ms | p99 ms | Server CPU µs/request |','| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |']
for r in summaries:lines.append(f"| {r['concurrency']} | {r['workload']} | {r['target']} | {r['throughput']:,.0f} | {r['p50Ms']:.4f} | {r['p95Ms']:.4f} | {r['p99Ms']:.4f} | {r['serverCpuUsPerRequest']:.1f} |")
(out/'table.md').write_text('\n'.join(lines)+'\n')
# Retain exact uncompressed evidence hashes in addition to source/artifact identities.
(raw/'atomic.json').write_text((raw/'atomic.log').read_text())
paths=['variants.json','selection.json','tests.log','native-test.log','negatives.json','mutation/evidence.json','atomic.json','http/smoke.json','http/results.json','micro/results.json','write-diagnostic/results.json','write-diagnostic.log']
local=json.loads((raw/'correctness.log').read_text())['evidence'];paths.append(str(Path(local).relative_to(raw)))
evidence=out/'evidence';evidence.mkdir(exist_ok=True);manifest=[]
for name in paths:
 b=(raw/name).read_bytes();dest=name.replace('/','-')+'.gz';(evidence/dest).write_bytes(gzip.compress(b,mtime=0));manifest.append({'file':'evidence/'+dest,'source':str((raw/name).relative_to(root)),'rawBytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(evidence/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
files=['environment.ts','server.ts','http.ts','micro.ts','variants.py','diagnostic-http.ts','diagnostic-server.ts','compiler/generate.py','compiler/runtime.rs','compiler/build/application.wasm','compiler/build/mutated.wasm','compiler/build/speed.wasm','compiler/build/ownership.wasm','compiler/build/combined.wasm']
artifacts={name:{'sha256':hashlib.sha256((out/name).read_bytes()).hexdigest(),'bytes':(out/name).stat().st_size} for name in files}
assert artifacts['http.ts']['sha256']==http['hashes']['runner'] and artifacts['server.ts']['sha256']==http['hashes']['server'] and artifacts['environment.ts']['sha256']==http['hashes']['environment'] and artifacts['compiler/build/application.wasm']['sha256']==http['hashes']['candidate']
(out/'artifacts.json').write_text(json.dumps(artifacts,indent=2)+'\n')
print(json.dumps({'httpRuns':len(http['results']),'httpRequests':report['httpRequests'],'microRuns':len(micro['results']),'microOperations':report['microOperations'],'errors':0},indent=2))
