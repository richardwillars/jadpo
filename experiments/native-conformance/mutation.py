#!/usr/bin/env python3
import pathlib,subprocess,json,hashlib,os
root=pathlib.Path.cwd();base=root/'experiments/native-conformance';out=root/'build/native-conformance/mutation';out.mkdir(parents=True,exist_ok=True)
source=(root/'experiments/wasm-exp1/fixture/app.jadpo').read_text();results=[]
def run(cmd,log,env=None):
 p=subprocess.run(cmd,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,env={**os.environ,**(env or {})});log.write_text(p.stdout);return p
original=hashlib.sha256((base/'build/application.rs').read_bytes()).hexdigest()
try:
 for name,text in [('renamed',source.replace('Item','Record').replace('User','Account').replace('owner_id','account_key')),('refined',source.replace('min_length: 3','min_length: 5'))]:
  folder=out/name;src=folder/'source';src.mkdir(parents=True,exist_ok=True);(src/'app.jadpo').write_text(text)
  p=run(['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(src),str(folder/'projected')],folder/'projection.log');assert p.returncode==0,p.stdout
  p=run(['sh',str(base/'build.sh'),str(folder/'projected/program.json')],folder/'build.log');assert p.returncode==0,p.stdout
  env={'NATIVE_EXPECT_REFINED':'1'} if name=='refined' else {}
  p=run(['cargo','test','--offline','--locked','--manifest-path',str(base/'Cargo.toml'),'--target-dir',str(root/'build/native-conformance/target')],folder/'rust-tests.log',env);assert p.returncode==0,p.stdout
  p=run(['bun','--no-install','--env-file=/dev/null','test',str(base/'wasm.test.ts')],folder/'wasm-tests.log',env);assert p.returncode==0,p.stdout
  results.append({'variant':name,'nativeAndWasmPassed':True,'generatedRustSha256':hashlib.sha256((base/'build/application.rs').read_bytes()).hexdigest(),'wasmSha256':hashlib.sha256((base/'build/application.wasm').read_bytes()).hexdigest()})
  (folder/'application.rs').write_bytes((base/'build/application.rs').read_bytes());(folder/'application.wasm').write_bytes((base/'build/application.wasm').read_bytes())
 p=json.loads((root/'experiments/wasm-exp1/compiler/build/projected/program.json').read_text())
 for name in ['freshness','cross_store','field_policy']:
  candidate=json.loads(json.dumps(p))
  if name=='freshness':candidate['queries'][0]['freshness']='eventual'
  elif name=='cross_store':candidate['transactions'][0]['domain']='elsewhere'
  else:candidate['policy']['entities'][0]['scopeField']='indirect'
  path=out/(name+'.json');path.write_text(json.dumps(candidate));r=run(['python3',str(base/'prepare.py'),str(path)],out/(name+'.log'));assert r.returncode!=0
  results.append({'variant':name,'rejected':True})
finally:
 r=run(['sh',str(base/'build.sh')],out/'restore.log');assert r.returncode==0,r.stdout
 assert hashlib.sha256((base/'build/application.rs').read_bytes()).hexdigest()==original
(out/'results.json').write_text(json.dumps({'variants':results,'originalSourceRestored':True},indent=2)+'\n');print(json.dumps(results))
