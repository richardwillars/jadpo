#!/usr/bin/env python3
import pathlib,subprocess,json,hashlib
root=pathlib.Path.cwd();base=root/'experiments/native-exp1';out=root/'build/native-exp1/mutation';out.mkdir(parents=True,exist_ok=True)
source=(root/'experiments/wasm-exp1/fixture/app.jadpo').read_text();results=[]
def run(cmd,env=None):
 import os
 p=subprocess.run(cmd,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,env={**os.environ,**(env or {})});return p
try:
 for name,text in [('renamed',source.replace('Item','Record').replace('User','Account').replace('owner_id','account_key')),('refined',source.replace('min_length: 3','min_length: 5'))]:
  folder=out/name;src=folder/'source';src.mkdir(parents=True,exist_ok=True);(src/'app.jadpo').write_text(text)
  p=run(['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(src),str(folder/'projected')]);(folder/'projection.log').write_text(p.stdout);assert p.returncode==0
  p=run(['python3',str(base/'prepare.py'),str(folder/'projected/program.json')]);assert p.returncode==0,p.stdout
  p=run(['cargo','test','--offline','--locked','--manifest-path',str(base/'Cargo.toml'),'--target-dir',str(root/'build/native-exp1/target'),'--','--test-threads=1'],{'NATIVE_EXPECT_REFINED':'1'} if name=='refined' else {})
  (folder/'tests.log').write_text(p.stdout);assert p.returncode==0,p.stdout
  results.append({'variant':name,'passed':True,'generatedSha256':hashlib.sha256((base/'build/generated.rs').read_bytes()).hexdigest()})
 # Same checked projection, deliberately unsupported authority/transaction plans.
 original=json.loads((root/'experiments/wasm-exp1/compiler/build/projected/program.json').read_text())
 for name in ['freshness','cross_store','field_policy']:
  p=json.loads(json.dumps(original))
  if name=='freshness':p['queries'][0]['freshness']='eventual'
  elif name=='cross_store':p['transactions'][0]['domain']='elsewhere'
  else:p['policy']['entities'][0]['scopeField']='indirect'
  path=out/(name+'.json');path.write_text(json.dumps(p));r=run(['python3',str(base/'prepare.py'),str(path)]);assert r.returncode!=0
  (out/(name+'.log')).write_text(r.stdout);results.append({'variant':name,'rejected':True})
finally:
 p=run(['python3',str(base/'prepare.py')]);assert p.returncode==0,p.stdout
 expected=json.loads((base/'evidence/freeze.json').read_text())['files']['experiments/wasm-exp1/workerd-guest-scans/compiler/build/generated.rs']
 assert hashlib.sha256((base/'build/generated.rs').read_bytes()).hexdigest()==expected
(out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results))
