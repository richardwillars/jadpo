from pathlib import Path
import subprocess,hashlib,json,shutil
r=Path(__file__).resolve().parents[3];o=r/'experiments/wasm-exp1/workerd-guest-scans';w=r/'build/wasm-exp1/workerd-guest-scans/mutation';w.mkdir(parents=True,exist_ok=True)
s=(r/'experiments/wasm-exp1/fixture/app.jadpo').read_text();assert s.count('min_length: 3')==1
variants={'mutated':s.replace('min_length: 3','min_length: 5'),'renamed':s.replace('Item','Record').replace('User','Account').replace('owner_id','account_key')}
def run(args):subprocess.run(args,cwd=r,check=True,stdout=subprocess.DEVNULL)
run(['sh',str(o/'build-guests.sh')])
artifact=o/'compiler/build/bulk.wasm';digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();original=digest(artifact);evidence=[]
try:
 for name,source_text in variants.items():
  source=w/name/'source';source.mkdir(parents=True,exist_ok=True);(source/'app.jadpo').write_text(source_text)
  run(['jadpo/target/debug/jadpo','check',str(source)])
  projection=w/name/'projected';run(['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(source),str(projection)])
  run(['sh',str(o/'build-guests.sh'),str(projection/'program.json')]);shutil.copyfile(artifact,o/f'compiler/build/{name}.wasm');shutil.copyfile(o/'compiler/build/row-codec.json',o/f'compiler/build/{name}-row-codec.json')
  evidence.append({'name':name,'sha256':digest(artifact),'checkedProjection':str(projection/'program.json')})
finally:
 run(['sh',str(o/'build-guests.sh')])
shutil.rmtree(o/'compiler/build/bulk-cargo');run(['sh',str(o/'build-guests.sh')]);assert digest(artifact)==original
(w/'evidence.json').write_text(json.dumps({'originalSha256':original,'variants':evidence,'cleanRebuildIdentical':True,'cargoCacheRemoved':True},indent=2)+'\n')
