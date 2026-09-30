import json,pathlib,statistics,collections
out=pathlib.Path('build/auth-policy-measure');dest=pathlib.Path('experiments/auth-policy-measure')
http=json.loads((out/'http.json').read_text());startup=json.loads((out/'startup.json').read_text());builds=json.loads((out/'build-times/results.json').read_text())
assert 'failure' not in http and 'failure' not in startup
assert len(http['results'])==252 and len(startup['results'])==30
med=statistics.median
groups=collections.defaultdict(list)
for r in http['results']:groups[(r['journal'],r['concurrency'],r['workload'],r['target'])].append(r)
metrics=['rps','inclusiveRps','p50Ms','p95Ms','p99Ms','cpuUsPerRequest','startRssBytes','endRssBytes']
summary=[]
for (journal,c,work,target),rows in sorted(groups.items()):
 assert len(rows)==3 and all(r['errors']==0 and r['snapshotVerified'] for r in rows)
 summary.append(dict(journal=journal,concurrency=c,workload=work,target=target,count=sum(r['count'] for r in rows),statementsPerRequest=rows[0]['statementsPerRequest'],**{k:med(r[k] for r in rows) for k in metrics},rpsRange=[min(r['rps'] for r in rows),max(r['rps'] for r in rows)],p95Range=[min(r['p95Ms'] for r in rows),max(r['p95Ms'] for r in rows)],maxMs=max(r['maxMs'] for r in rows),peakRssBytes=max(r['processAccounting']['peakRssBytes'] for r in rows)))
start=[]
for target in ['bun','native','wasm']:
 rows=[r for r in startup['results'] if r['target']==target]
 start.append(dict(target=target,**{key:med(r[key] for r in rows) for key in ['readyMs','firstHttpMs','spawnToFirstMs','rssBytes']},spawnToFirstRange=[min(r['spawnToFirstMs'] for r in rows),max(r['spawnToFirstMs'] for r in rows)]))
result={'machine':http['machine'],'protocol':http['protocol'],'httpCells':len(http['results']),'measuredRequests':sum(r['count'] for r in http['results']),'warmupRequests':32*len(http['results']),'startupRequests':len(startup['results']),'aggregation':'median of three per-run metrics; maximum of run maxima and lifetime RSS peaks; ranges retained; no cross-journal pooling','http':summary,'startup':start,'builds':builds}
(dest/'results.json').write_text(json.dumps(result,indent=2)+'\n')
lines=['# Complete authenticated benchmark table','','Three repetitions per row. Latencies in ms; CPU in µs/request; RSS in MiB. Throughput is requests/s. RSS is median post-phase resident memory; peak is the largest process-lifetime peak. CPU includes trace drains; throughput excludes them (inclusive rate is also shown). These are Bun-hosted WASM measurements.','','| Journal | Clients | Workload | Target | Count | Active rps | Inclusive rps | p50 | p95 | p99 | Max | CPU | RSS | Peak | SQL/request |','|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|']
for r in summary:lines.append(f"| {r['journal']} | {r['concurrency']} | {r['workload']} | {r['target']} | {r['count']} | {r['rps']:.0f} | {r['inclusiveRps']:.0f} | {r['p50Ms']:.3f} | {r['p95Ms']:.3f} | {r['p99Ms']:.3f} | {r['maxMs']:.1f} | {r['cpuUsPerRequest']:.1f} | {r['endRssBytes']/2**20:.1f} | {r['peakRssBytes']/2**20:.1f} | {r['statementsPerRequest']} |")
(dest/'table.md').write_text('\n'.join(lines)+'\n')
print(json.dumps({'requests':result['measuredRequests'],'startup':start},indent=2))
