"""Verify the frozen comparison and archive all selection rounds, including losses."""
from pathlib import Path
import gzip, hashlib, json, statistics
here=Path(__file__).resolve().parent
root=here.parents[2]
work=root/'build/wasm-exp1/workerd-native-values'
read=lambda n:json.loads((work/n).read_text())
http,smoke,micro,selection=[read(n) for n in ['results.json','smoke.json','micro.json','selection.json']]
assert len(http['results'])==200 and len(smoke['results'])==20
assert len(micro['results'])==120
assert http['manifest']==smoke['manifest']==micro['manifest']
assert http['manifest']['hashes']['candidate']==selection['moduleSha256']
assert http['manifest']['hashes']['worker']==selection['bundleSha256']
for name,digest in selection['hashes'].items():
    assert hashlib.sha256((here/name).read_bytes()).hexdigest()==digest,name
for doc in [http,smoke]:
    assert len(doc['preflight'])==3 and all(p['passed']==7 and not p['runtime']['hasBun'] for p in doc['preflight'])
    assert all(r['errors']==0 and r['snapshotVerified'] for r in doc['results'])
    for r in doc['results']:
        for key in ['poolStats','newPoolStats','nextPoolStats','hybridPoolStats']:
            assert r[key]['active']==0 and r[key]['discarded']==0 and r[key]['maxRetainedPages']<=32
for name,count in [('first-selection/micro.json',140),('second-selection/micro.json',270),('micro.json',120)]:
    doc=read(name)
    assert len(doc['results'])==count
    assert all(r['snapshotVerified'] and r['iterations']==2000 for r in doc['results'])
    assert len({(r['run'],r['target'],r['workload']) for r in doc['results']})==count
median=statistics.median
summaries,pairs,headroom=[],[],[]
for c in [1,16]:
    for w in http['protocol']['workloads']:
        cells={t:[r for r in http['results'] if r['target']==t and r['concurrency']==c and r['workload']==w] for t in http['protocol']['targets']}
        assert all(len(rs)==5 and {r['run'] for r in rs}==set(range(1,6)) for rs in cells.values())
        summaries.append({'concurrency':c,'workload':w,'targets':{t:{k:{'median':median(r[k] for r in rs),'min':min(r[k] for r in rs),'max':max(r[k] for r in rs)} for k in ['throughput','p95Ms','p99Ms','maxMs']} for t,rs in cells.items()}})
        for r in cells['hybrid']:
            for baseline in ['views','js']:
                b=next(x for x in cells[baseline] if x['run']==r['run'])
                pairs.append({'concurrency':c,'workload':w,'baseline':baseline,'run':r['run'],'throughputRatio':r['throughput']/b['throughput'],'p95Ratio':r['p95Ms']/b['p95Ms'],'p99Ratio':r['p99Ms']/b['p99Ms']})
        headroom.append({'concurrency':c,'workload':w,'ratios':{t:median(next(x['throughput'] for x in cells['noop'] if x['run']==r['run'])/r['throughput'] for r in rs) for t,rs in cells.items() if t!='noop'}})
paired=[]
for c in [1,16]:
    for w in http['protocol']['workloads']:
        for b in ['views','js']:
            rows=[r for r in pairs if r['concurrency']==c and r['workload']==w and r['baseline']==b]
            paired.append({'concurrency':c,'workload':w,'baseline':b,**{k:median(r[k] for r in rows) for k in ['throughputRatio','p95Ratio','p99Ratio']},'throughputWins':sum(r['throughputRatio']>1 for r in rows),'p95Wins':sum(r['p95Ratio']<1 for r in rows),'p99Wins':sum(r['p99Ratio']<1 for r in rows)})
retention={t:max(r[key]['maxRetainedPages'] for r in http['results']) for t,key in [('previous','newPoolStats'),('selected','hybridPoolStats')]}
iso={t:{w:{'medianUs':median(r['externalUs'] for r in micro['results'] if r['target']==t and r['workload']==w),'frameBytes':next(r['frameBytes'] for r in micro['results'] if r['workload']==w)} for w in micro['protocol']['workloads']} for t in micro['protocol']['targets']}
result={'scope':http['protocol']['scope'],'requests':sum(r['count'] for r in http['results']),'isolated':iso,'summary':summaries,'pairs':pairs,'pairedMedians':paired,'headroom':headroom,'capacityConclusive':all(v>=2 for h in headroom for v in h['ratios'].values()),'maxRetainedPages':retention,'selection':selection,'cpuMeasured':False,'hostedColdStartsMeasured':False}
(here/'results.json').write_text(json.dumps(result,indent=2)+'\n')
lines=['# Workerd native-values HTTP results','','Five paired repetitions; medians. `views` is the frozen previous JSON driver; `hybrid` is the sampled candidate.','','| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms |','| --- | --- | --- | ---: | ---: | ---: |']
for s in summaries:
    for t,v in s['targets'].items():
        lines.append(f'| {s["concurrency"]} | {s["workload"]} | {t} | {v["throughput"]["median"]:.0f} | {v["p95Ms"]["median"]:.4f} | {v["p99Ms"]["median"]:.4f} |')
(here/'table.md').write_text('\n'.join(lines)+'\n')
evidence=here/'evidence';evidence.mkdir(exist_ok=True)
files={n:work/n for n in ['results.json','smoke.json','micro.json','selection.json','provenance.json','tests.log','http.log','smoke.log','micro.log','tests-initial.log']}
for folder in ['first-selection','second-selection']:
    for p in sorted((work/folder).iterdir()):files[str(p.relative_to(work))]=p
# All final source plus the exact bundle and both unique module byte sequences.
for n in ['worker.mjs','manifest.json','candidate.wasm','previous.wasm','wrangler.jsonc']:files['bundle/'+n]=work/'bundle'/n
for p in sorted(here.iterdir()):
    if p.suffix in ['.ts','.mjs']:files['source/'+p.name]=p
manifest=[]
for n,p in files.items():
    data=p.read_bytes();target=n.replace('/','-')+'.gz';(evidence/target).write_bytes(gzip.compress(data,mtime=0));manifest.append({'source':n,'file':target,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
(evidence/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
for entry in manifest:
    assert hashlib.sha256(gzip.decompress((evidence/entry['file']).read_bytes())).hexdigest()==entry['sha256']
print(json.dumps({k:result[k] for k in ['requests','pairedMedians','capacityConclusive','maxRetainedPages']},indent=2))
