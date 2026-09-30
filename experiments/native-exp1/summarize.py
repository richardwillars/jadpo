#!/usr/bin/env python3
import json,pathlib,statistics as s
base=pathlib.Path(__file__).resolve().parent;out=base.parents[1]/'build/native-exp1'
result={}
for name in ['http','delete','micro','startup']:
 p=out/(name+'.json')
 if not p.exists():continue
 raw=json.loads(p.read_text());rows=raw['results'];groups={}
 # Measured Bun 1.2.20 on Darwin already reports maxRSS in bytes.
 # Preserve untouched raw files/logs; normalize the initial wrapper's KiB assumption.
 for x in rows:
  if x['target']=='bun' and 'peakRssBytes' in x: x['peakRssBytes']/=raw['protocol'].get('bunPeakRssDivisor',1024)
 for x in rows:
  key=(x['target'],x.get('concurrency',1),x.get('workload','startup'))
  groups.setdefault(key,[]).append(x)
 metrics=['throughput','p50Ms','p95Ms','p99Ms','maxMs','cpuUsPerRequest','rssEndBytes','peakRssBytes'] if name in ['http','delete'] else ['throughput','p50Us','p95Us','p99Us','maxUs','cpuUs'] if name=='micro' else ['readyMs','firstHttpMs','processToFirstResponseMs','warmP50Ms','rssBytes']
 summaries=[]
 for (target,c,w),xs in groups.items():
  summaries.append({'target':target,'concurrency':c,'workload':w,'repetitions':len(xs),'metrics':{k:{'median':s.median(x[k] for x in xs),'min':min(x[k] for x in xs),'max':max(x[k] for x in xs)} for k in metrics}})
 paired=[]
 for (_,c,w),xs in groups.items():
  if xs[0]['target']!='native':continue
  bun={x['run']:x for x in groups[('bun',c,w)]}
  ratios={}
  for k in metrics:
   rr=[x[k]/bun[x['run']][k] for x in xs];ratios[k]={'median':s.median(rr),'min':min(rr),'max':max(rr),'nativeWins':sum(v>1 if k=='throughput' else v<1 for v in rr)}
  paired.append({'concurrency':c,'workload':w,'nativeOverBun':ratios})
 result[name]={'cells':len(rows),'verifiedCount':sum(x.get('count',1) for x in rows),'errors':sum(x.get('errors',0) for x in rows),'groups':summaries,'paired':paired}
 if name=='http':
  headroom=[]
  for x in rows:
   if x['workload'].startswith('read_'):
    noop=next(n for n in rows if n['target']==x['target'] and n['run']==x['run'] and n['concurrency']==x['concurrency'] and n['workload']==x['workload'].replace('read_','noop_'))
    headroom.append({'target':x['target'],'run':x['run'],'concurrency':x['concurrency'],'workload':x['workload'],'ratio':noop['throughput']/x['throughput']})
  result[name]['headroom']=headroom
(base/'results.json').write_text(json.dumps(result,indent=2)+'\n')
lines=['# Native experiment measurements','','Medians; raw ranges and paired ratios are in `results.json`. HTTP rates are local closed-loop observations, not capacity.','']
for name in ['http','delete']:
 if name not in result:continue
 lines += [f'## {name.upper()}','','| Workload | C | Target | req/s | p50 ms | p95 ms | p99 ms | max ms¹ | CPU µs/req | RSS MiB |','|---|---:|---|---:|---:|---:|---:|---:|---:|---:|']
 for x in sorted(result[name]['groups'],key=lambda x:(x['concurrency'],x['workload'],x['target'])):
  m=x['metrics'];lines.append(f"| {x['workload']} | {x['concurrency']} | {x['target']} | {m['throughput']['median']:.0f} | {m['p50Ms']['median']:.3f} | {m['p95Ms']['median']:.3f} | {m['p99Ms']['median']:.3f} | {m['maxMs']['median']:.3f} | {m['cpuUsPerRequest']['median']:.2f} | {m['rssEndBytes']['median']/1048576:.2f} |")
 lines+=['','¹ Median of each cell’s maximum, not the maximum over the campaign. Full maxima remain in the raw results.','']
(base/'table.md').write_text('\n'.join(lines)+'\n')
print(json.dumps({k:{kk:v[kk] for kk in ['cells','verifiedCount','errors']} for k,v in result.items()}))
