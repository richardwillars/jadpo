"""Package the changed compiler-owned host lifetime, preserving application bytes."""
from pathlib import Path
import subprocess,json,hashlib,shutil
r=Path(__file__).resolve().parents[3];o=r/'experiments/wasm-exp1/optimization';out=r/'build/wasm-exp1/optimization/cloud'
subprocess.run(['python3',str(r/'experiments/wasm-exp1/full/build-cloud.py'),str(o/'compiler/build/application.wasm'),str(out),'--mutated',str(o/'compiler/build/mutated.wasm')],check=True)
shutil.copyfile(o/'driver.ts',out/'host/driver.ts');shutil.copyfile(o/'cached-adapter.ts',out/'storage/adapter.ts')
s=(o/'cached-host.ts').read_text().replace("'./driver.ts'","'../host/driver.ts'").replace("'./cached-adapter.ts'","'../storage/adapter.ts'");(out/'full/host.ts').write_text(s)
s=(out/'worker.js').read_text();s='import {poolStats} from "./host/driver.ts";\n'+s;s=s.replace('  reset(){return this.host.reset();}','  statistics(){return {...poolStats};}\n  reset(){return this.host.reset();}');s=s.replace('return Response.json({reportId,metadata,probe,full}', 'return Response.json({reportId,metadata,probe,full,poolStats:{outer:{...poolStats},authority:await stub.statistics()}}');(out/'worker.js').write_text(s)
p=out/'wrangler.jsonc';d=json.loads(p.read_text());d['name']='jadpo-wasm-opt1-20260930';p.write_text(json.dumps(d,indent=2)+'\n')
p=out/'package-evidence.json';d=json.loads(p.read_text());d['extension']={'instanceLifetime':'Exclusive lease; reset before reuse; discard internal/trap/invalid protocol;8idle max,32 retained pages max','host':'Cached immutable policy/node plan; Cloudflare SQL adapter unchanged. Local Bun prepared-statement cache is not used on Cloudflare.','sourceFiles':{name:hashlib.sha256((o/name).read_bytes()).hexdigest() for name in ['driver.ts','cached-host.ts','cached-adapter.ts']}};p.write_text(json.dumps(d,indent=2)+'\n')
