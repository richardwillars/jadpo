"""Verify complete qualification, retain every pair/loss, and archive raw evidence."""
from pathlib import Path
import json,statistics,hashlib,gzip
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
work=root/'build/wasm-exp1/read-path';median=statistics.median
read=lambda n:json.loads((work/n).read_text())
http=read('http/results.json');smoke=read('http/smoke.json');writes=read('http-writes/smoke.json');matrix=read('matrix/results.json');startup=read('startup.json');selection=read('selection.json')
assert len(http['results'])==80 and len(http['servers'])==40
assert len(matrix['results'])==240 and len(startup['results'])==40
assert len(writes['results'])==24 and len(smoke['results'])==32
for document in [http,smoke,writes]:
 assert document['hashes']['candidate']==selection['candidateSha256']
 assert document['hashes']['previous']==selection['previousSha256']
 assert all(r['errors']==0 and r['snapshotVerified'] and r['count']==r['server']['count'] for r in document['results'])
 assert all(len(s['preflight'])==8 and all(p['status']=='pass' for p in s['preflight']) for s in document['servers'])
assert all(r['errors']==0 for r in matrix['results']) and all(r['verified'] for r in startup['results'])
def cpu(r):return sum(r['server']['cpu'].values())/r['count']
def metric(r,k):return cpu(r) if k=='cpuUs' else r[{'rps':'throughput','p95Ms':'p95Ms','p99Ms':'p99Ms','maxMs':'maxMs'}[k]]
def cell(rows):return {k:{'median':median(metric(r,k) for r in rows),'min':min(metric(r,k) for r in rows),'max':max(metric(r,k) for r in rows)} for k in ['rps','p95Ms','p99Ms','maxMs','cpuUs']}
summary=[];pairs=[];headroom=[]
for c in [1,16]:
 for w in http['protocol']['workloads']:
  cells={t:[r for r in http['results'] if r['target']==t and r['concurrency']==c and r['workload']==w] for t in ['bun','previous','candidate','noop']}
  summary.append({'concurrency':c,'workload':w,'targets':{t:cell(rs) for t,rs in cells.items()}})
  for r in cells['candidate']:
   for baseline in ['bun','previous']:
    b=next(x for x in cells[baseline] if x['run']==r['run'])
    pairs.append({'concurrency':c,'workload':w,'baseline':baseline,'run':r['run'],'rps':r['throughput']/b['throughput'],'p95':r['p95Ms']/b['p95Ms'],'cpu':cpu(r)/cpu(b)})
  headroom.append({'concurrency':c,'workload':w,'ratios':{t:median(next(n['throughput'] for n in cells['noop'] if n['run']==r['run'])/r['throughput'] for r in cells[t]) for t in ['bun','previous','candidate']}})
large=[p for p in pairs if p['workload']=='read_large' and p['baseline']=='bun']
parity={str(c):{k:{'median':median(p[k] for p in large if p['concurrency']==c),'pairsInside':sum((p[k]>=.9 if k=='rps' else p[k]<=1.1) for p in large if p['concurrency']==c)} for k in ['rps','p95','cpu']} for c in [1,16]}
small=[p for p in pairs if p['workload']=='read_small' and p['baseline']=='previous']
small_gate={str(c):{k:median(p[k] for p in small if p['concurrency']==c) for k in ['rps','p95','cpu']} for c in [1,16]}
parity_pass=all(v['pairsInside']>=4 and (v['median']>=.9 if k=='rps' else v['median']<=1.1) for c in parity.values() for k,v in c.items())
small_pass=all(v>=.95 if k=='rps' else v<=1.05 for c in small_gate.values() for k,v in c.items())
starts={t:{k:{'median':median(r[k] for r in startup['results'] if r['target']==t),'min':min(r[k] for r in startup['results'] if r['target']==t),'max':max(r[k] for r in startup['results'] if r['target']==t)} for k in ['readyMs','firstHttpMs','processToFirstResponseMs']} for t in ['bun','candidate']}
mat=[]
for operation in ['Item.read','Item.read_title']:
 for size in [256,4096,16384,49152]:
  for kind in ['ascii','escaped']:
   rows=[r for r in matrix['results'] if r['operation']==operation and r['size']==size and r['kind']==kind]
   targets={t:{k:median(r[k] for r in rows if r['target']==t) for k in ['throughput','p95Ms','cpuUs']} for t in ['bun','previous','candidate']}
   paired={t:{k:median(r[k]/next(b[k] for b in rows if b['target']==t and b['run']==r['run']) for r in rows if r['target']=='candidate') for k in ['throughput','p95Ms','cpuUs']} for t in ['bun','previous']}
   mat.append({'operation':operation,'nominalSize':size,'kind':kind,'frameBytes':rows[0]['frameBytes'],'targets':targets,'pairedRatios':paired})
pools=[r['server']['poolStats'] for r in http['results'] if r['target']=='candidate']
assert all(p['active']==0 and p['maxRetainedPages']<=32 and p['maxIdle']<=8 for p in pools)
result={'scope':'Local paired descriptive measurements; shared host and no-work ceiling preclude maximum-capacity claims. Selected artifact not hosted on Cloudflare.','selection':selection,'readRequests':sum(r['count'] for r in http['results']),'writeSmokeRequests':sum(r['count'] for r in writes['results']),'summary':summary,'pairs':pairs,'largeRowParity':parity,'largeRowParityPass':parity_pass,'smallRegression':small_gate,'smallRegressionPass':small_pass,'noopHeadroom':headroom,'capacityConclusive':all(v>=2 for h in headroom for v in h['ratios'].values()),'startup':starts,'matrix':mat,'retention':{'maxRetainedPages':max(p['maxRetainedPages'] for p in pools),'peakActive':max(p['peakActive'] for p in pools),'maxIdle':max(p['maxIdle'] for p in pools),'discarded':sum(p['discarded'] for p in pools),'note':'Pool snapshots are cumulative per process; summed discards may repeat between its two workloads.'}}
(out/'results.json').write_text(json.dumps(result,indent=2)+'\n')
lines=['# Large-read HTTP results','','Five paired repetitions; medians. Every pair and range is retained in results.json.','','| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms | CPU µs/request |','| --- | --- | --- | ---: | ---: | ---: | ---: |']
for item in summary:
 for t,v in item['targets'].items():lines.append(f'| {item["concurrency"]} | {item["workload"]} | {t} | {v["rps"]["median"]:.0f} | {v["p95Ms"]["median"]:.4f} | {v["p99Ms"]["median"]:.4f} | {v["cpuUs"]["median"]:.1f} |')
(out/'table.md').write_text('\n'.join(lines)+'\n')
evidence=out/'evidence';evidence.mkdir(exist_ok=True);manifest=[]
def archive(name,data):
 target=evidence/(name.replace('/','-')+'.gz');target.write_bytes(gzip.compress(data,mtime=0));manifest.append({'source':name,'file':target.name,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
correctness=read('correctness.log')
names=['profile.json','profile-revised.json','snapshot-profile.json','micro/results.json','micro-selected/results.json','matrix/results.json','http/results.json','http/smoke.json','http-writes/smoke.json','startup.json','selection.json','provenance.json','mutation/evidence.json','negatives.json','atomic.json','tests.log','rust-tests.log','correctness.log',str(Path(correctness['evidence']).relative_to(work))]
for name in names:archive(name,(work/name).read_bytes())
for stage in ['initial','revision-one']:
 for path in sorted((work/stage).iterdir()):archive(str(path.relative_to(work)),path.read_bytes())
for name in ['compiler/build/application.wasm','compiler/build/row-codec.json','compiler/build/manifest.json']:archive(name,(out/name).read_bytes())
(evidence/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['readRequests','writeSmokeRequests','largeRowParity','smallRegression','capacityConclusive','startup','retention']},indent=2))
