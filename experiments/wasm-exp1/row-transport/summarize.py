"""Verify completeness and produce paired summaries without hiding losing runs."""
from pathlib import Path
import json,statistics,hashlib,gzip
root=Path(__file__).resolve().parents[3]
work=root/'build/wasm-exp1/row-transport'
out=Path(__file__).resolve().parent
median=statistics.median
def read(name):return json.loads((work/name).read_text())
http=read('http/results.json');writes=read('http-writes/results.json');startup=read('startup.json')
assert len(http['results'])==80 and len(writes['results'])==36 and len(startup['results'])==40
assert len(http['servers'])==40 and len(writes['servers'])==18
for document in [http,writes]:
 assert all(r['errors']==0 and r['snapshotVerified'] and r['count']==r['server']['count'] for r in document['results'])
 assert all(len(s['preflight'])==8 and all(p['status']=='pass' for p in s['preflight']) for s in document['servers'])
def cpu(r):return sum(r['server']['cpu'].values())/r['count']
def cell(rows):return {'rps':median(r['throughput'] for r in rows),'p95Ms':median(r['p95Ms'] for r in rows),'p99Ms':median(r['p99Ms'] for r in rows),'cpuUs':median(cpu(r) for r in rows)}
summary=[];pairs=[]
for document in [http,writes]:
 rows=document['results']
 for concurrency in [1,16]:
  for workload in document['protocol']['workloads']:
   cells={t:[r for r in rows if r['target']==t and r['concurrency']==concurrency and r['workload']==workload] for t in ['bun','previous','candidate','noop']}
   summary.append({'concurrency':concurrency,'workload':workload,'targets':{t:cell(rs) for t,rs in cells.items() if rs}})
   for baseline in ['bun','previous']:
    for candidate in cells['candidate']:
     reference=next(r for r in cells[baseline] if r['run']==candidate['run'])
     ratios={'rps':candidate['throughput']/reference['throughput'],'p95':candidate['p95Ms']/reference['p95Ms'],'cpu':cpu(candidate)/cpu(reference)}
     pairs.append({'concurrency':concurrency,'workload':workload,'baseline':baseline,'run':candidate['run'],**ratios})
large=[p for p in pairs if p['workload']=='read_large' and p['baseline']=='bun']
parity={str(c):{k:{'median':median(p[k] for p in large if p['concurrency']==c),'pairsInside':sum((p[k]>=.9 if k=='rps' else p[k]<=1.1) for p in large if p['concurrency']==c)} for k in ['rps','p95','cpu']} for c in [1,16]}
small=[p for p in pairs if p['workload']=='read_small' and p['baseline']=='previous']
small_gate={str(c):{k:median(p[k] for p in small if p['concurrency']==c) for k in ['rps','p95','cpu']} for c in [1,16]}
starts={t:{k:{'median':median(r[k] for r in startup['results'] if r['target']==t),'min':min(r[k] for r in startup['results'] if r['target']==t),'max':max(r[k] for r in startup['results'] if r['target']==t)} for k in ['readyMs','firstHttpMs','processToFirstResponseMs']} for t in ['bun','candidate']}
result={'scope':'Local descriptive paired measurements; no-work ceiling and uncontrolled workstation prevent maximum-capacity claims. Latest module not hosted on Cloudflare.','readRequests':sum(r['count'] for r in http['results']),'writeRequests':sum(r['count'] for r in writes['results']),'summary':summary,'pairs':pairs,'largeRowParity':parity,'smallRegression':small_gate,'startup':starts}
(out/'results.json').write_text(json.dumps(result,indent=2)+'\n')
lines=['# Row transport HTTP results','','Medians of paired runs. CPU is server process CPU per request. See results.json for every pair and gate.','','| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms | CPU µs/request |','| --- | --- | --- | ---: | ---: | ---: | ---: |']
for item in summary:
 for target,values in item['targets'].items():lines.append(f'| {item["concurrency"]} | {item["workload"]} | {target} | {values["rps"]:.0f} | {values["p95Ms"]:.4f} | {values["p99Ms"]:.4f} | {values["cpuUs"]:.1f} |')
(out/'table.md').write_text('\n'.join(lines)+'\n')
evidence=out/'evidence';evidence.mkdir(exist_ok=True)
names=['profile.json','profile-candidate.json','initial-micro.json','micro/results.json','micro-selected/results.json','matrix/results.json','http/results.json','http/smoke.json','http-writes/results.json','http-interrupted.json','startup.json','selection.json','mutation/evidence.json','negatives.json','atomic.json','tests.log','rust-tests.log','correctness.log','initial-row-codec.rs','initial-driver.ts','initial-codec.ts','initial.wasm','pre-fallback.wasm']
manifest=[]
for name in names:
 data=(work/name).read_bytes();target=evidence/(name.replace('/','-')+'.gz');target.write_bytes(gzip.compress(data,mtime=0));manifest.append({'source':name,'file':target.name,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
for name in ['compiler/build/application.wasm','compiler/build/row-codec.json','compiler/build/manifest.json']:
 data=(out/name).read_bytes();target=evidence/(Path(name).name+'.gz');target.write_bytes(gzip.compress(data,mtime=0));manifest.append({'source':name,'file':target.name,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
(evidence/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'requests':result['readRequests']+result['writeRequests'],'parity':parity,'small':small_gate,'startup':starts},indent=2))
