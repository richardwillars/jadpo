from pathlib import Path
import json,hashlib,gzip,statistics
h=Path(__file__).resolve().parent;r=h.parents[2];w=r/'build/wasm-exp1/workerd-shared-rows'
x=json.loads((w/'micro.json').read_text());assert len(x['results'])==120
assert len(x['preflight'])==3 and all(p['passed']==7 and not p['runtime']['hasBun'] for p in x['preflight'])
assert all(c['iterations']==2000 and c['snapshotVerified'] for c in x['results'])
assert len({(c['target'],c['run'],c['workload']) for c in x['results']})==120
for key in ['worker','bulk','shared']:
 p=w/'bundle'/('worker.mjs' if key=='worker' else key+'.wasm');assert hashlib.sha256(p.read_bytes()).hexdigest()==x['manifest']['hashes'][key]
summary={t:{work:{'medianUs':statistics.median(c['externalUs'] for c in x['results'] if c['target']==t and c['workload']==work),'valuesUs':[c['externalUs'] for c in x['results'] if c['target']==t and c['workload']==work]} for work in x['protocol']['workloads']} for t in x['protocol']['targets']}
(h/'results.json').write_text(json.dumps({'selected':False,'reason':'Only ~1–2% isolated gains, slight large-ASCII loss; retain simpler frozen bulk-memory candidate. No HTTP qualification.','measuredCalls':240000,'summary':summary},indent=2)+'\n')
e=h/'evidence';e.mkdir(exist_ok=True)
files={n:w/n for n in ['micro.json','micro.log','tests.log','native-tests.log','build.log','mutation.log']}
for n in ['worker.mjs','manifest.json','shared.wasm','bulk.wasm','candidate.wasm']:files['bundle/'+n]=w/'bundle'/n
for p in h.rglob('*'):
 if p.is_file() and 'build' not in p.relative_to(h).parts and p.suffix in ['.ts','.mjs','.py','.sh','.rs','.toml','.lock']:files['source/'+str(p.relative_to(h))]=p
for n in ['generated.rs','manifest.json','row-codec.json','mutated.wasm','renamed.wasm','mutated-row-codec.json','renamed-row-codec.json']:files['generated/'+n]=h/'compiler/build'/n
files['mutation-evidence.json']=w/'mutation/evidence.json'
manifest=[]
for n,p in files.items():
 b=p.read_bytes();name=n.replace('/','-')+'.gz';(e/name).write_bytes(gzip.compress(b,mtime=0));manifest.append({'source':n,'file':name,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(e/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
for f in manifest:assert hashlib.sha256(gzip.decompress((e/f['file']).read_bytes())).hexdigest()==f['sha256']
print('Verified 120 cells, 240,000 calls and',len(manifest),'evidence files; shared variant unselected')
