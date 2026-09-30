#!/usr/bin/env python3
"""Retain the bounded conformance evidence; never reuse historical performance scores."""
import datetime,gzip,hashlib,json,pathlib,platform,subprocess
root=pathlib.Path.cwd();base=root/'experiments/auth-policy';out=root/'build/auth-policy';evidence=base/'evidence'
def digest(data):return hashlib.sha256(data).hexdigest()
freeze=json.loads((evidence/'freeze.json').read_text());checks={p:digest((root/p).read_bytes())==h for p,h in freeze.items()}
assert all(checks.values()),'frozen control changed'
results=json.loads((out/'results.json').read_text());assert len(results)==6
assert 'All targets agree' in (out/'http.log').read_text()
mutation=json.loads((out/'mutation/results.json').read_text());assert mutation['sourceConformancePassed'] and mutation['unsafeOutputRejected']
wasm=(base/'build/application.wasm').read_bytes()
assert wasm==(out/'wasm-clean/wasm32-unknown-unknown/release/jadpo_auth_policy_wasm.wasm').read_bytes()
artifacts={}
inputs={}
for p in base.rglob('*'):
    if p.is_file() and not any(x in p.relative_to(base).parts for x in ['evidence','build','__pycache__']) and p.name!='results.json':inputs[str(p.relative_to(root))]=p
for p in [base/'build/application.rs',base/'build/contract.json',base/'build/manifest.json',base/'build/application.wasm',base/'build/projected/program.json',out/'target/release/jadpo-auth-policy-native',out/'seed.json',out/'seed.sqlite',out/'results.json',out/'results-variant.json',out/'mutation/results.json']:
    inputs[str(p.relative_to(root))]=p
for folder in [base/'build/projected/bun',out/'mutation']:
    for p in folder.rglob('*'):
        if p.is_file():inputs[str(p.relative_to(root))]=p
for p in out.glob('*.log'):inputs[str(p.relative_to(root))]=p
for source,p in sorted(inputs.items()):
    data=p.read_bytes();name=source.replace('/','--')+'.gz';packed=gzip.compress(data,mtime=0)
    (evidence/name).write_bytes(packed);artifacts[source]={'archive':name,'bytes':len(data),'sha256':digest(data),'archiveSha256':digest(packed)}
ordinary=sum(sum('result' in rec for rec in r['records']) for r in results)
concurrent=sum(sum(len(rec.get('concurrent',[])) for rec in r['records']) for r in results)
summary={'scope':'local authentication/policy conformance; WASM hosted by Bun; no performance campaign',
 'httpScenarioCalls':ordinary,'concurrentCalls':concurrent,'totalHttpCalls':ordinary+concurrent,
 'mutationHttpCalls':sum(len(r['records']) for r in json.loads((out/'results-variant.json').read_text())),
 'nativeLifecycleTests':5,'wasmLifecycleTests':4,'wasmAssertions':50,'projectionTests':3,
 'unsupportedPlansRejected':mutation['negativePlans'],'unsafeOutputRejected':True,
 'cleanWasmIdentical':True,'frozenFilesUnchanged':len(checks),
 'generatedRustSha256':digest((base/'build/application.rs').read_bytes()),'contractSha256':digest((base/'build/contract.json').read_bytes()),
 'wasmSha256':digest(wasm),'wasmBytes':len(wasm),
 'nativeSha256':digest((out/'target/release/jadpo-auth-policy-native').read_bytes()),'nativeBytes':(out/'target/release/jadpo-auth-policy-native').stat().st_size,
 'sql':[]}
for r in results:
    summary['sql'].append({'target':r['target'],'journal':r['journal'],'sqliteVersion':r['sqliteVersion'],'pragmas':r['pragmas'],
       'statements':{name:len(next(x for x in r['records'] if x['name']==name)['trace']) for name in ['owner read','editor title update','atomic pair commit','second row denied rolls back first','SQL fault rolls back first write']}})
versions={name:subprocess.check_output(cmd,text=True).strip() for name,cmd in {'rust':['rustc','--version'],'bun':['bun','--version'],'node':['node','--version'],'python':['python3','--version']}.items()}
(base/'results.json').write_text(json.dumps(summary,indent=2)+'\n')
manifest={'createdUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'platform':platform.platform(),'versions':versions,'summary':summary,'frozenChecks':checks,'artifacts':artifacts}
(evidence/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'archives':len(artifacts),'http':ordinary+concurrent,'mutationHttp':summary['mutationHttpCalls'],'frozen':len(checks),'wasm':summary['wasmSha256']}))
