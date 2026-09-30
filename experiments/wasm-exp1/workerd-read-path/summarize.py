"""Retain the whole local workerd campaign, including paired losses and limits."""
from pathlib import Path
import gzip, hashlib, json, statistics

here = Path(__file__).resolve().parent
root = here.parents[2]
work = root / 'build/wasm-exp1/workerd-read-path'
read = lambda name: json.loads((work / name).read_text())
data = read('results.json')
smoke = read('smoke.json')
assert len(data['results']) == 100 and len(smoke['results']) == 10
assert data['manifest'] == smoke['manifest']
assert data['manifest']['hashes']['candidate'] == 'cf3716529f29fd0ababa6e4d99982908ba7f9326035b486f9c03d1ffe2d99c0b'
for document in [data, smoke]:
    assert len(document['preflight']) == 4
    assert all(p['passed'] == 7 and not p['runtime']['hasBun'] for p in document['preflight'])
    assert all(r['errors'] == 0 and r['snapshotVerified'] for r in document['results'])
    assert all(r['poolStats']['active'] == 0 and r['poolStats']['discarded'] == 0 and r['poolStats']['maxRetainedPages'] <= 32 for r in document['results'])

median = statistics.median
summary, pairs, headroom = [], [], []
for concurrency in [1, 16]:
    for workload in ['small', 'large']:
        cells = {target: [r for r in data['results'] if r['target'] == target and r['concurrency'] == concurrency and r['workload'] == workload] for target in data['protocol']['targets']}
        summary.append({'concurrency': concurrency, 'workload': workload, 'targets': {t: {k: {'median': median(r[k] for r in rows), 'min': min(r[k] for r in rows), 'max': max(r[k] for r in rows)} for k in ['throughput', 'p95Ms', 'p99Ms', 'maxMs']} for t, rows in cells.items()}})
        for target in ['candidate', 'typed', 'previous']:
            for r in cells[target]:
                for baseline in ['js', 'previous']:
                    if target == baseline:
                        continue
                    b = next(x for x in cells[baseline] if x['run'] == r['run'])
                    pairs.append({'concurrency': concurrency, 'workload': workload, 'target': target, 'baseline': baseline, 'run': r['run'], 'throughputRatio': r['throughput'] / b['throughput'], 'p95Ratio': r['p95Ms'] / b['p95Ms']})
        headroom.append({'concurrency': concurrency, 'workload': workload, 'ratios': {t: median(next(x['throughput'] for x in cells['noop'] if x['run'] == r['run']) / r['throughput'] for r in rows) for t, rows in cells.items() if t != 'noop'}})

result = {'scope': data['protocol']['scope'], 'requests': sum(r['count'] for r in data['results']), 'cells': len(data['results']), 'summary': summary, 'pairs': pairs, 'noopHeadroom': headroom, 'capacityConclusive': all(v >= 2 for h in headroom for v in h['ratios'].values()), 'cpuMeasured': False, 'coldStartsMeasured': False, 'manifest': data['manifest']}
(here / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
lines = ['# Local workerd read results', '', 'Five rotated repetitions; medians. Generated JS and WASM share workerd and the SQLite authority.', '', '| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms |', '| --- | --- | --- | ---: | ---: | ---: |']
for item in summary:
    for target, values in item['targets'].items():
        lines.append(f'| {item["concurrency"]} | {item["workload"]} | {target} | {values["throughput"]["median"]:.0f} | {values["p95Ms"]["median"]:.4f} | {values["p99Ms"]["median"]:.4f} |')
(here / 'table.md').write_text('\n'.join(lines) + '\n')
evidence = here / 'evidence'
evidence.mkdir(exist_ok=True)
manifest = []
sources = {name: work / name for name in ['results.json', 'smoke.json', 'run.log', 'smoke.log', 'setup-failure.json', 'bundle/manifest.json', 'bundle/worker.mjs', 'bundle/candidate.wasm', 'bundle/typed.wasm', 'bundle/previous.wasm', 'bundle/wrangler.jsonc', 'worker-configuration.d.ts']}
sources['baseline-generated-app.ts'] = root / 'build/wasm-exp1/baseline/generated/bun/target/app.ts'
for name in ['worker.ts', 'runner.mjs', 'build.mjs', 'persistence-port.ts']:
    sources['source/' + name] = here / name
for name, source in sources.items():
    content = source.read_bytes()
    destination = name.replace('/', '-') + '.gz'
    (evidence / destination).write_bytes(gzip.compress(content, mtime=0))
    manifest.append({'source': name, 'file': destination, 'sha256': hashlib.sha256(content).hexdigest(), 'bytes': len(content)})
(evidence / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({k: result[k] for k in ['requests', 'cells', 'capacityConclusive']}, indent=2))
print('\n'.join(lines))
