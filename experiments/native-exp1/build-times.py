#!/usr/bin/env python3
"""Run sequentially after all performance campaigns; warm filesystem caches."""
import pathlib,subprocess,time,json,hashlib,os,platform
base=pathlib.Path('experiments/native-exp1');out=pathlib.Path('build/native-exp1/build-times');out.mkdir(parents=True,exist_ok=True);results=[]
def timed(name,cmd):
 start=time.perf_counter();p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True);elapsed=time.perf_counter()-start
 (out/(name+'.log')).write_text(p.stdout);results.append({'name':name,'command':cmd,'seconds':elapsed,'exitCode':p.returncode});assert p.returncode==0,p.stdout
cargo=['cargo','build','--offline','--locked','--release','--manifest-path',str(base/'Cargo.toml')]
original=hashlib.sha256(pathlib.Path('build/native-exp1/target/release/jadpo-native-exp1').read_bytes()).hexdigest()
for rep in range(3):
 timed(f'bun-check-generate-{rep}', ['experiments/wasm-exp1/compiler/target/debug/jadpo-wasm-projection','experiments/wasm-exp1/fixture',str(out/f'bun-{rep}'),'--bun'])
 timed(f'native-lower-{rep}',['python3',str(base/'prepare.py')])
 timed(f'native-recompile-{rep}',cargo+['--target-dir','build/native-exp1/target'])
 timed(f'native-noop-{rep}',cargo+['--target-dir','build/native-exp1/target'])
# A fresh Cargo target directory, while source/dependency download and OS caches
# remain warm. It is deliberately not the ongoing measurement executable.
clean=out/f'clean-{os.getpid()}'
timed('native-clean',cargo+['--target-dir',str(clean)])
new=hashlib.sha256((clean/'release/jadpo-native-exp1').read_bytes()).hexdigest()
metadata={'platform':platform.platform(),'machine':platform.machine(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'cargo':subprocess.check_output(['cargo','--version'],text=True).strip(),'bun':subprocess.check_output(['bun','--version'],text=True).strip(),'node':subprocess.check_output(['node','--version'],text=True).strip(),'originalBinarySha256':original,'cleanBinarySha256':new,'cleanBinaryIdentical':original==new,'results':results}
(out/'results.json').write_text(json.dumps(metadata,indent=2)+'\n');print(json.dumps(metadata))
