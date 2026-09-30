from pathlib import Path
import subprocess,shutil,os,hashlib,json,time,gzip
root=Path(__file__).resolve().parents[3];p=Path(__file__).resolve().parent;b=p/'compiler/build';out=root/'build/wasm-exp1/value-path'
env=os.environ.copy();env['RUSTFLAGS']='-C target-feature=-simd128,-atomics,-bulk-memory,-reference-types,-multivalue -C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144'
source=(b/'generated.rs').read_bytes();old=(root/'experiments/wasm-exp1/boundary-http/compiler/build/generated.rs').read_bytes()
records=[]
for name,code,level in [('ownership',source,'s'),('speed',old,'3'),('combined',source,'3')]:
 (b/'generated.rs').write_bytes(code);env['CARGO_PROFILE_RELEASE_OPT_LEVEL']=level;start=time.monotonic()
 command=['cargo','build','--offline','--locked','--manifest-path',str(p/'compiler/Cargo.toml'),'--target','wasm32-unknown-unknown','--target-dir',str(b/'cargo'),'--release']
 run=subprocess.run(command,cwd=root,env=env,capture_output=True);(out/(name+'-build.log')).write_bytes(run.stdout+run.stderr);assert run.returncode==0,(name,run.stderr.decode())
 wasm=(b/'cargo/wasm32-unknown-unknown/release/jadpo_wasm_exp1_rust_full.wasm').read_bytes();(b/(name+'.wasm')).write_bytes(wasm)
 records.append({'name':name,'optLevel':level,'generatedRustSha256':hashlib.sha256(code).hexdigest(),'wasmSha256':hashlib.sha256(wasm).hexdigest(),'bytes':len(wasm),'gzipBytes':len(gzip.compress(wasm,mtime=0)),'buildSeconds':time.monotonic()-start,'rustflags':env['RUSTFLAGS']})
(b/'generated.rs').write_bytes(source);shutil.copyfile(b/'combined.wasm',b/'application.wasm');(out/'variants.json').write_text(json.dumps(records,indent=2)+'\n');print(json.dumps(records,indent=2))
