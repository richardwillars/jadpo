from pathlib import Path
import subprocess,hashlib,json,shutil
r=Path(__file__).resolve().parents[3];o=r/'experiments/wasm-exp1/value-path';w=r/'build/wasm-exp1/value-path/mutation';w.mkdir(parents=True,exist_ok=True)
source=w/'source';source.mkdir(exist_ok=True);s=(r/'experiments/wasm-exp1/fixture/app.jadpo').read_text();assert s.count('min_length: 3')==1;(source/'app.jadpo').write_text(s.replace('min_length: 3','min_length: 5'))
def run(args):subprocess.run(args,cwd=r,check=True,stdout=subprocess.DEVNULL)
artifact=o/'compiler/build/application.wasm';digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();original=digest(artifact)
run(['jadpo/target/debug/jadpo','check',str(source)])
run(['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection',str(source),str(w/'projected')])
run(['sh',str(o/'build.sh'),str(w/'projected/program.json')]);shutil.copyfile(artifact,o/'compiler/build/mutated.wasm');mutant=digest(artifact)
shutil.rmtree(o/'compiler/build/cargo');run(['sh',str(o/'build.sh')]);assert digest(artifact)==original
(w/'evidence.json').write_text(json.dumps({'originalSha256':original,'mutantSha256':mutant,'cleanRebuildIdentical':True,'sourceMutation':'ItemTitle min_length3→5, ordinary check+projection before generation','cargoCacheRemoved':True},indent=2)+'\n')
