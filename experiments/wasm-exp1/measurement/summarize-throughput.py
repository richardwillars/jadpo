#!/usr/bin/env python3
"""Retain complete repeated-run evidence; never summarize partial runs as final."""
import hashlib,json,statistics
from pathlib import Path
root=Path(__file__).resolve().parents[3];here=Path(__file__).resolve().parent
source=root/'build/wasm-exp1/throughput/results.json';d=json.loads(source.read_text());runs=d['results']
assert len(runs)==40, f'Expected40complete runs, got{len(runs)}'
assert all(r['errors']==0 and r['requests']>0 and r['seconds']>=30 for r in runs)
summary=[];pairs=[]
for mode in ['cpu','io']:
 for concurrency in [1,8]:
  group={target:sorted([r for r in runs if r['target']==target and r['mode']==mode and r['concurrency']==concurrency],key=lambda r:r['run']) for target in ['bun','wasm']}
  assert all([r['run'] for r in xs]==[1,2,3,4,5] for xs in group.values())
  for target,xs in group.items():
   row={'target':target,'mode':mode,'concurrency':concurrency,'runs':len(xs),'requests':sum(r['requests'] for r in xs),'errors':sum(r['errors'] for r in xs)}
   for metric in ['successfulThroughput','p50Ms','p95Ms','p99Ms','rssBytes']:
    values=[r[metric] for r in xs];row[metric]={'median':statistics.median(values),'min':min(values),'max':max(values)}
   summary.append(row)
  ratios=[{'run':a['run'],'p50RatioWasmOverBun':b['p50Ms']/a['p50Ms'],'p95RatioWasmOverBun':b['p95Ms']/a['p95Ms'],'throughputRatioWasmOverBun':b['successfulThroughput']/a['successfulThroughput']} for a,b in zip(group['bun'],group['wasm'])]
  pairs.append({'mode':mode,'concurrency':concurrency,'pairedRatios':ratios,'cpuBenefitRunCount':sum(r['p50RatioWasmOverBun']<=.8 and r['throughputRatioWasmOverBun']>=1 for r in ratios),'ioWithin20PercentAllRuns':all(r['p95RatioWasmOverBun']<=1.2 for r in ratios)})
current=root/'experiments/wasm-exp1/rust-full/build/probe.wasm';assert hashlib.sha256(current.read_bytes()).hexdigest()==d['wasmHash']
d['runnerSha256']=hashlib.sha256((here/'throughput.ts').read_bytes()).hexdigest();d['summary']=summary;d['comparisons']=pairs
d['qualifications']=['Warmup semantic results are computed but failures not counted; timed failures are counted and reject run.','Wholeprocess RSS includes both target support and latency arrays; not isolated target memory.','Uncontrolled workstation, thermals and OS caches; early runs overlap short compiler/test work.','Readonly synchronous SQLite workload; concurrency means outstanding requests, not parallel CPU execution.','Fresh isolated Bun fixture DB has one matching table; runner chooses first shape match without asserting uniqueness.','No confidence interval or statistical significance claim.','Fresh Wasm instance and JSON copies per request are included; no general Wasm performance claim.']
(here/'throughput-results.json').write_text(json.dumps(d,indent=2)+'\n')
lines=['| Workload / concurrency | Target | Throughput/s median (range) | p50 ms median | p95 ms median (range) | p99 ms median |','| --- | --- | ---: | ---: | ---: | ---: |']
for r in summary:
 t=r['successfulThroughput'];p=r['p95Ms'];lines.append(f"| {r['mode']} / {r['concurrency']} | {r['target']} | {t['median']:.0f} ({t['min']:.0f}–{t['max']:.0f}) | {r['p50Ms']['median']:.6f} | {p['median']:.6f} ({p['min']:.6f}–{p['max']:.6f}) | {r['p99Ms']['median']:.6f} |")
(here/'throughput-table.md').write_text('\n'.join(lines)+'\n');print('\n'.join(lines));print('totalrequests',sum(r['requests'] for r in runs))
